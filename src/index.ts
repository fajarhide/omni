import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import fs from "fs";
import path from "path";
import { promisify } from "util";
import { exec, execFile } from "child_process";
import { fileURLToPath } from "url";
import { WASI } from "wasi";
import { LRUCache } from "./cache.js";
import os from "os";
import crypto from "crypto";

const execAsync = promisify(exec);
const execFileAsync = promisify(execFile);
const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Parse Agent Argument (--agent=claude-code)
let currentAgent = "unknown";
for (const arg of process.argv) {
  if (arg.startsWith("--agent=")) {
    currentAgent = arg.split("=")[1] || "unknown";
  }
}

// Local Metrics Logic
const METRICS_FILE = path.join(os.homedir(), ".omni", "metrics.csv");

async function logMetrics(filterName: string, inputLen: number, outputLen: number, ms: number) {
  try {
    const timestamp = Math.floor(Date.now() / 1000);
    const line = `${timestamp},${currentAgent},${filterName},${inputLen},${outputLen},${Math.round(ms)}\n`;
    
    // Ensure .omni directory exists
    const dir = path.dirname(METRICS_FILE);
    if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });

    await fs.promises.appendFile(METRICS_FILE, line);
  } catch (e) {
    // Ignore logging errors to remain transparent
  }
}

const server = new Server(
  {
    name: "omni-server",
    version: "0.4.4",
  },
  {
    capabilities: {
      tools: {},
    },
  }
);

const CACHE_CAPACITY = 100;
const CACHE_TTL = 3600000; // 1 hour
const cache = new LRUCache<string, string>(CACHE_CAPACITY, CACHE_TTL);

const WASM_PATH_DEFAULT = path.join(__dirname, "..", "core", "omni-wasm.wasm");
const WASM_PATH_BUILD = path.join(__dirname, "..", "core", "zig-out", "bin", "omni-wasm.wasm");
const WASM_PATH = fs.existsSync(WASM_PATH_BUILD) ? WASM_PATH_BUILD : WASM_PATH_DEFAULT;

// Config Discovery: Global & Local
const GLOBAL_CONFIG_DIR = path.join(os.homedir(), ".omni");
const GLOBAL_CONFIG_PATH = path.join(GLOBAL_CONFIG_DIR, "omni_config.json");
const LOCAL_CONFIG_PATH = path.join(process.cwd(), "omni_config.json");

// Security: Hook Integrity Check
const HOOKS_DIR = path.join(os.homedir(), ".omni", "hooks");
const HOOKS_SHA_FILE = path.join(os.homedir(), ".omni", "hooks.sha256");

async function verifyHookIntegrity() {
  if (!fs.existsSync(HOOKS_SHA_FILE)) return;

  try {
    const storedHashesRaw = fs.readFileSync(HOOKS_SHA_FILE, "utf-8");
    const storedHashes: Record<string, string> = JSON.parse(storedHashesRaw);
    
    if (!fs.existsSync(HOOKS_DIR)) {
      if (Object.keys(storedHashes).length > 0) {
        console.error(`Security Alert: Hooks directory ${HOOKS_DIR} missing but hashes exist.`);
        process.exit(1);
      }
      return;
    }

    const files = fs.readdirSync(HOOKS_DIR);
    const currentHashes: Record<string, string> = {};

    for (const file of files) {
      const filePath = path.join(HOOKS_DIR, file);
      if (fs.statSync(filePath).isDirectory()) continue;
      
      const content = fs.readFileSync(filePath);
      const hash = crypto.createHash('sha256').update(content).digest('hex');
      currentHashes[file] = hash;
    }

    // Check for mismatches or missing files
    for (const [file, hash] of Object.entries(storedHashes)) {
      if (currentHashes[file] !== hash) {
        console.error(`Security Alert: Hook integrity mismatch for file: ${file}`);
        process.exit(1);
      }
    }

    // Check for new untrusted files
    for (const file of Object.keys(currentHashes)) {
      if (!storedHashes[file]) {
        console.error(`Security Alert: New untrusted hook file detected: ${file}`);
        process.exit(1);
      }
    }
  } catch (e: any) {
    console.error(`Error verifying hook integrity: ${e.message}`);
    process.exit(1);
  }
}

function getMergedConfig(): any {
  let config: any = { rules: [], dsl_filters: [] };

  // 1. Load Global Config
  if (fs.existsSync(GLOBAL_CONFIG_PATH)) {
    try {
      const globalRaw = fs.readFileSync(GLOBAL_CONFIG_PATH, "utf-8");
      const globalConfig = JSON.parse(globalRaw);
      if (globalConfig.rules) config.rules.push(...globalConfig.rules);
      if (globalConfig.dsl_filters) config.dsl_filters.push(...globalConfig.dsl_filters);
    } catch (e) {
      console.error("Error parsing global config:", e);
    }
  }

  // 2. Merge Local Config (Overrides/Augments)
  if (fs.existsSync(LOCAL_CONFIG_PATH)) {
    try {
      const localRaw = fs.readFileSync(LOCAL_CONFIG_PATH, "utf-8");
      const localConfig = JSON.parse(localRaw);
      // For now, just append rules/filters. Specific name-matching overrides can be added later.
      if (localConfig.rules) config.rules.push(...localConfig.rules);
      if (localConfig.dsl_filters) config.dsl_filters.push(...localConfig.dsl_filters);
    } catch (e) {
      console.error("Error parsing local config:", e);
    }
  }

  return config;
}

const TEMPLATES: Record<string, any[]> = {
  "kubernetes": [
    { name: "k8s_uid", match: "uid:", action: "mask" },
    { name: "k8s_managed_fields", match: "managedFields:", action: "remove" }
  ],
  "terraform": [
    { name: "tf_refresh", match: "Refreshing state...", action: "remove" },
    { name: "tf_no_changes", match: "No changes. Your infrastructure matches the configuration.", action: "mask" }
  ],
  "node-verbose": [
    { name: "npm_notice", match: "npm notice", action: "remove" },
    { name: "node_modules_path", match: "node_modules/", action: "mask" }
  ],
  "docker-layers": [
    { name: "docker_hash", match: "sha256:", action: "mask" }
  ],
  "security-audit": [
    { name: "ip_mask", match: "192.168.", action: "mask" },
    { name: "password_remove", match: "password:", action: "remove" },
    { name: "key_mask", match: "PRIVATE KEY", action: "mask" }
  ],
  "aws-cloud": [
    { name: "aws_request_id", match: "RequestId:", action: "remove" },
    { name: "aws_arn_mask", match: "arn:aws:", action: "mask" }
  ]
};

let wasmInstance: WebAssembly.Instance | null = null;
let wasi: WASI | null = null;

async function getWasmInstance() {
  if (wasmInstance) return wasmInstance;

  wasi = new WASI({
    version: "preview1",
    args: [],
    env: process.env,
    preopens: {
      ".": path.dirname(WASM_PATH),
    },
  });

  const wasmBuffer = fs.readFileSync(WASM_PATH);
  const { instance } = await WebAssembly.instantiate(wasmBuffer, {
    wasi_snapshot_preview1: wasi.wasiImport,
  });

  wasmInstance = instance;
  
  // Initialize WASI
  const exports = wasmInstance.exports as any;
  if (exports._start) {
    wasi.start(wasmInstance);
  } else if (wasi.initialize) {
    wasi.initialize(wasmInstance);
  }

  // Initial engine bootstrap (can be empty, will be re-initialized per request if needed
  // but better to load existing configs once at startup)
  const config = getMergedConfig();
  const configStr = JSON.stringify(config);
  const encoder = new TextEncoder();
  const configBytes = encoder.encode(configStr);
  
  const ptr = exports.alloc(configBytes.length);
  const memory = exports.memory as WebAssembly.Memory;
  const memView = new Uint8Array(memory.buffer);
  memView.set(configBytes, ptr);

  if (exports.init_engine_with_config && !exports.init_engine_with_config(ptr, configBytes.length)) {
    console.error("Failed to initialize OMNI engine in Wasm with config");
  } else if (!exports.init_engine_with_config && exports.init_engine && !exports.init_engine()) {
     console.error("Failed to initialize OMNI engine in Wasm (legacy)");
  }

  exports.free(ptr, configBytes.length);

  return wasmInstance;
}

// Helper: Distill text via OMNI Wasm engine
async function distillText(text: string): Promise<string> {
  const startTime = performance.now();
  const cached = cache.get(text);
  if (cached) {
    // We don't store filter name in cache for now, so we use 'cache' as name
    await logMetrics("cache", text.length, cached.length, performance.now() - startTime);
    return cached;
  }

  const instance = await getWasmInstance();
  const exports = instance.exports as any;
  const memory = exports.memory as WebAssembly.Memory;

  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  
  const inputBytes = encoder.encode(text);
  const inputPtr = exports.alloc(inputBytes.length);
  
  if (!inputPtr) throw new Error("Wasm allocation failed");

  let resultPtr = 0;
  let resultLen = 0;

  try {
    const memView = new Uint8Array(memory.buffer);
    memView.set(inputBytes, inputPtr);

    // Call compress: returns struct { ptr, len }
    const resultRaw = exports.compress(inputPtr, inputBytes.length);
    resultPtr = Number(BigInt(resultRaw) & 0xFFFFFFFFn);
    resultLen = Number(BigInt(resultRaw) >> 32n);

    const outputBytes = new Uint8Array(memory.buffer, resultPtr, resultLen);
    const output = decoder.decode(outputBytes);

    // Retrieve Filter Name
    const namePtr = exports.get_last_filter_name_ptr();
    const nameLen = exports.get_last_filter_name_len();
    const nameBytes = new Uint8Array(memory.buffer, namePtr, nameLen);
    const filterName = decoder.decode(nameBytes);

    const trimmed = output.trim();
    cache.set(text, trimmed);
    
    const elapsed = performance.now() - startTime;
    await logMetrics(filterName, text.length, trimmed.length, elapsed);
    
    return trimmed;
  } finally {
    exports.free(inputPtr, inputBytes.length);
    if (resultPtr) exports.free(resultPtr, resultLen);
  }
}

async function discoverFilters(text: string): Promise<any[]> {
  const instance = await getWasmInstance();
  const exports = instance.exports as any;
  const memory = exports.memory as WebAssembly.Memory;

  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  
  const inputBytes = encoder.encode(text);
  const inputPtr = exports.alloc(inputBytes.length);
  
  if (!inputPtr) throw new Error("Wasm allocation failed");

  let resultPtr = 0;
  let resultLen = 0;

  try {
    const memView = new Uint8Array(memory.buffer);
    memView.set(inputBytes, inputPtr);

    // Call discover
    const resultRaw = exports.discover(inputPtr, inputBytes.length);
    resultPtr = Number(BigInt(resultRaw) & 0xFFFFFFFFn);
    resultLen = Number(BigInt(resultRaw) >> 32n);

    const outputBytes = new Uint8Array(memory.buffer, resultPtr, resultLen);
    const jsonStr = decoder.decode(outputBytes);
    
    return JSON.parse(jsonStr);
  } catch (e) {
    console.error("Discovery failed:", e);
    return [];
  } finally {
    exports.free(inputPtr, inputBytes.length);
    if (resultPtr) exports.free(resultPtr, resultLen);
  }
}

server.setRequestHandler(ListToolsRequestSchema, async () => {
  return {
    tools: [
      {
        name: "omni_compress",
        description: "Compress a raw string to save LLM tokens using Zig-powered OMNI Wasm engine.",
        inputSchema: {
          type: "object",
          properties: {
            text: { type: "string", description: "The raw text to compress" },
          },
          required: ["text"],
        },
      },
      {
        name: "omni_execute",
        description: "Execute a shell command (e.g., git diff, docker build, npm install) and automatically distill the output through OMNI. Use this instead of running commands directly to save massive tokens.",
        inputSchema: {
          type: "object",
          properties: {
            command: { type: "string", description: "The shell command to execute" },
            cwd: { type: "string", description: "Optional working directory for the command" },
          },
          required: ["command"],
        },
      },
      {
        name: "omni_read_file",
        description: "Read a local file and automatically distill its contents through OMNI. Great for huge logs or SQL files.",
        inputSchema: {
          type: "object",
          properties: {
            path: { type: "string", description: "Absolute path to the file to read" },
          },
          required: ["path"],
        },
      },
      {
        name: "omni_density",
        description: "Analyze a piece of text to measure the Context Density Gain and token reduction ratio when using OMNI.",
        inputSchema: {
          type: "object",
          properties: {
            text: { type: "string", description: "The raw text to analyze" },
          },
          required: ["text"],
        },
      },
      {
        name: "omni_list_dir",
        description: "List the contents of a directory. Returns a highly dense comma-separated string of files and folders (e.g. DIR:src, FILE:package.json). Used exclusively to save tokens instead of standard JSON explorers.",
        inputSchema: {
          type: "object",
          properties: {
            path: { type: "string", description: "Absolute path to the directory" },
          },
          required: ["path"],
        },
      },
      {
        name: "omni_view_file",
        description: "View the contents of a file (or specific line ranges) and compress it via OMNI's Zig engine to remove noise. Lines are 1-indexed.",
        inputSchema: {
          type: "object",
          properties: {
            path: { type: "string", description: "Absolute path to the file" },
            startLine: { type: "number", description: "Optional starting line number (1-indexed)" },
            endLine: { type: "number", description: "Optional ending line number (1-indexed)" }
          },
          required: ["path"],
        },
      },
      {
        name: "omni_grep_search",
        description: "Search for a pattern within a directory or file using grep. Returns highly dense matching output to save tokens.",
        inputSchema: {
          type: "object",
          properties: {
            path: { type: "string", description: "Absolute path to search within (directory or file)" },
            query: { type: "string", description: "Search term or regex pattern" },
            isRegex: { type: "boolean", description: "Whether the query is a regular expression (default: false)" },
            caseInsensitive: { type: "boolean", description: "Perform case-insensitive search (default: false)" }
          },
          required: ["path", "query"],
        },
      },
      {
        name: "omni_find_by_name",
        description: "Recursively find files in a directory by name (uses find command). Returns a dense comma-separated list of matches.",
        inputSchema: {
          type: "object",
          properties: {
            dir: { type: "string", description: "Absolute path to the directory to search" },
            pattern: { type: "string", description: "Glob pattern to match file names (e.g. '*.ts')" }
          },
          required: ["dir", "pattern"],
        },
      },
      {
        name: "omni_add_filter",
        description: "Add a new declarative filter rule to OMNI without coding. Rules are saved to omni_config.json and applied instantly.",
        inputSchema: {
          type: "object",
          properties: {
            name: { type: "string", description: "Name of the filter rule" },
            match: { type: "string", description: "Text pattern to match in tool output" },
            action: { type: "string", enum: ["remove", "mask"], description: "Action to take: 'remove' (silent delete) or 'mask' (replace with [MASKED])" }
          },
          required: ["name", "match", "action"],
        },
      },
      {
        name: "omni_apply_template",
        description: "Apply a bundle of pre-defined filter rules for common technology stacks.",
        inputSchema: {
          type: "object",
          properties: {
            template: { 
              type: "string", 
              enum: ["kubernetes", "terraform", "node-verbose", "docker-layers"],
              description: "The template to apply" 
            }
          },
          required: ["template"],
        },
      },
      {
        name: "Bash",
        description: "[OMNI AUTOPILOT] Execute a shell command. Automatically distills output to save massive tokens. (Replaces Claude's default Bash tool)",
        inputSchema: {
          type: "object",
          properties: {
            command: { type: "string" }
          },
          required: ["command"],
        },
      },
      {
        name: "ReadFile",
        description: "[OMNI AUTOPILOT] Read a local file and automatically distill its contents through OMNI. (Replaces Claude's default ReadFile tool)",
        inputSchema: {
          type: "object",
          properties: {
            file_path: { type: "string" }
          },
          required: ["file_path"],
        },
      },
      {
        name: "run_command",
        description: "[OMNI AUTOPILOT] Execute a terminal command. Automatically distills output to save huge tokens. (Replaces Antigravity's default run_command tool)",
        inputSchema: {
          type: "object",
          properties: {
            CommandLine: { type: "string" },
            Cwd: { type: "string" }
          },
          required: ["CommandLine"],
        },
      },
      {
        name: "view_file",
        description: "[OMNI AUTOPILOT] View the contents of a file and compress it via OMNI. (Replaces Antigravity's default view_file tool)",
        inputSchema: {
          type: "object",
          properties: {
            AbsolutePath: { type: "string" },
            StartLine: { type: "number" },
            EndLine: { type: "number" }
          },
          required: ["AbsolutePath"],
        },
      },
      {
        name: "omni_trust",
        description: "Verify and store SHA-256 hashes of hook scripts in ~/.omni/hooks. Run this after you manually inspect and approve new or modified hooks.",
        inputSchema: {
          type: "object",
          properties: {},
          required: [],
        },
      },
      {
        name: "omni_learn",
        description: "Analyze raw tool output to discover repetitive noise patterns and suggest new OMNI filters. Use this to 'teach' OMNI about new types of noise.",
        inputSchema: {
          type: "object",
          properties: {
            text: { type: "string", description: "The raw output to analyze (e.g. from a build or log)" },
            apply: { type: "boolean", description: "If true, automatically apply the suggested filters (use with caution!)" }
          },
          required: ["text"],
        },
      }
    ],
  };
});

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  try {
    switch (request.params.name) {
      
      case "omni_compress": {
        const text = (request.params.arguments as any).text;
        const distilled = await distillText(text);
        return { content: [{ type: "text", text: distilled }] };
      }

      case "omni_execute": {
        const args = request.params.arguments as any;
        const opts = args.cwd ? { cwd: args.cwd } : {};
        let resultOutput = "";
        let exitCode = 0;
        
        try {
          const { stdout, stderr } = await execAsync(args.command, opts);
          resultOutput = String(stdout) + (stderr ? "\nSTDERR:\n" + String(stderr) : "");
        } catch (e: any) {
          resultOutput = String(e.stdout || "") + (e.stderr ? "\nSTDERR:\n" + String(e.stderr) : "") + `\nExit code: ${e.code}`;
          exitCode = e.code || 1;
        }
        
        const distilled = await distillText(resultOutput);
        return { 
          content: [{ type: "text", text: distilled }],
          metadata: { exitCode }
        };
      }

      case "omni_read_file": {
        const filePath = (request.params.arguments as any).path;
        const rawContent = await fs.promises.readFile(filePath, "utf-8");
        const distilled = await distillText(rawContent);
        return { content: [{ type: "text", text: distilled }] };
      }

      case "omni_density": {
        const text = (request.params.arguments as any).text;
        const distilled = await distillText(text);
        
        const originalLen = text.length;
        const distilledLen = distilled.length;
        const gain = distilledLen > 0 ? (originalLen / distilledLen).toFixed(2) : "Infinity";
        const reduction = originalLen > 0 ? ((1 - (distilledLen / originalLen)) * 100).toFixed(1) : "0.0";
        
        const report = 
          `Original length:  ${originalLen} chars\n` +
          `Distilled length: ${distilledLen} chars\n` +
          `Reduction:        ${reduction}%\n` +
          `Density Gain:     ${gain}x`;
          
        return { content: [{ type: "text", text: report }] };
      }

      case "omni_list_dir": {
        const dirPath = (request.params.arguments as any).path;
        try {
          const entries = await fs.promises.readdir(dirPath, { withFileTypes: true });
          const denseEntries = entries.map(ent => {
            if (ent.isDirectory()) return `DIR:${ent.name}`;
            return `FILE:${ent.name}`;
          });
          const resultOutput = `[${dirPath}]\n${denseEntries.join(", ")}`;
          // Distill won't do much on short CSVs, but good for consistency
          const distilled = await distillText(resultOutput);
          return { content: [{ type: "text", text: distilled }] };
        } catch (e: any) {
          return { content: [{ type: "text", text: `Error reading dir: ${e.message}` }], isError: true };
        }
      }

      case "omni_view_file": {
        const args = request.params.arguments as any;
        const filePath = args.path;
        const startLine = args.startLine ? Math.max(1, args.startLine) : 1;
        const endLine = args.endLine ? Math.max(startLine, args.endLine) : Infinity;

        try {
          const rawContent = await fs.promises.readFile(filePath, "utf-8");
          const lines = rawContent.split("\n");
          const targetLines = lines.slice(startLine - 1, endLine === Infinity ? undefined : endLine);
          
          let chunk = targetLines.join("\n");
          if (startLine > 1 || endLine < lines.length) {
             chunk = `[Showing lines ${startLine}-${Math.min(endLine, lines.length)} of ${filePath}]\n${chunk}`;
          }
          const distilled = await distillText(chunk);
          return { content: [{ type: "text", text: distilled }] };
        } catch (e: any) {
           return { content: [{ type: "text", text: `Error reading file: ${e.message}` }], isError: true };
        }
      }

      case "omni_grep_search": {
         const args = request.params.arguments as any;
         const targetPath = args.path;
         const query = args.query;
         const isRegex = args.isRegex === true;
         const flags = ["-rn"];
         if (args.caseInsensitive) flags.push("-i");
         if (isRegex) flags.push("-E");

         try {
            const { stdout, stderr } = await execFileAsync('grep', [...flags, query, targetPath]);
            let resultOutput = String(stdout);
            if (!resultOutput.trim()) resultOutput = "No matches found.";
            
            const distilled = await distillText(resultOutput);
            return { 
              content: [{ type: "text", text: distilled }],
              metadata: { exitCode: 0 }
            };
         } catch (e: any) {
             if (e.code === 1) {
                 return { 
                   content: [{ type: "text", text: "No matches found." }],
                   metadata: { exitCode: 1 }
                 };
             }
             return { 
               content: [{ type: "text", text: `Grep error: ${e.message}` }], 
               isError: true,
               metadata: { exitCode: e.code || 1 }
             };
         }
      }

      case "omni_find_by_name": {
         const args = request.params.arguments as any;
         const dir = args.dir;
         const pattern = args.pattern || "*";
         
         try {
             // Basic find command
             const { stdout } = await execFileAsync('find', [dir, '-name', pattern]);
             const files = String(stdout).split("\n").filter(Boolean);
             const resultOutput = files.join(", ");
             const distilled = await distillText(resultOutput || "No files found.");
             return { 
               content: [{ type: "text", text: distilled }],
               metadata: { exitCode: 0 }
             };
         } catch (e: any) {
             return { 
               content: [{ type: "text", text: `Find error: ${e.message}` }], 
               isError: true,
               metadata: { exitCode: e.code || 1 }
             };
         }
      }

      case "omni_add_filter": {
        const args = request.params.arguments as any;
        try {
          const targetPath = fs.existsSync(LOCAL_CONFIG_PATH) ? LOCAL_CONFIG_PATH : GLOBAL_CONFIG_PATH;
          let config: any = { rules: [], dsl_filters: [] };
          
          if (fs.existsSync(targetPath)) {
            const raw = await fs.promises.readFile(targetPath, "utf-8");
            config = JSON.parse(raw);
          } else {
            // Ensure global directory exists if target is global
            if (targetPath === GLOBAL_CONFIG_PATH && !fs.existsSync(GLOBAL_CONFIG_DIR)) {
               fs.mkdirSync(GLOBAL_CONFIG_DIR, { recursive: true });
            }
          }

          if (!config.rules) config.rules = [];
          config.rules.push({
            name: args.name,
            match: args.match,
            action: args.action
          });
          
          await fs.promises.writeFile(targetPath, JSON.stringify(config, null, 2));
          
          // Force re-init of Wasm engine with new config
          wasmInstance = null; 
          
          return { content: [{ type: "text", text: `Filter '${args.name}' added successfully to ${targetPath}.` }] };
        } catch (e: any) {
          return { content: [{ type: "text", text: `Error adding filter: ${e.message}` }], isError: true };
        }
      }

      case "omni_apply_template": {
        const templateName = (request.params.arguments as any).template;
        const templateRules = TEMPLATES[templateName];
        if (!templateRules) throw new Error(`Template not found: ${templateName}`);

        try {
          const targetPath = fs.existsSync(LOCAL_CONFIG_PATH) ? LOCAL_CONFIG_PATH : GLOBAL_CONFIG_PATH;
          let config: any = { rules: [], dsl_filters: [] };
          
          if (fs.existsSync(targetPath)) {
            const raw = await fs.promises.readFile(targetPath, "utf-8");
            config = JSON.parse(raw);
          } else {
            if (targetPath === GLOBAL_CONFIG_PATH && !fs.existsSync(GLOBAL_CONFIG_DIR)) {
               fs.mkdirSync(GLOBAL_CONFIG_DIR, { recursive: true });
            }
          }
          
          if (!config.rules) config.rules = [];
          
          // Merge rules, avoid duplicates by name
          for (const rule of templateRules) {
            if (!config.rules.find((r: any) => r.name === rule.name)) {
              config.rules.push(rule);
            }
          }

          await fs.promises.writeFile(targetPath, JSON.stringify(config, null, 2));
          
          // Force re-init
          wasmInstance = null;

          return { content: [{ type: "text", text: `Template '${templateName}' applied successfully to ${targetPath}.` }] };
        } catch (e: any) {
          return { content: [{ type: "text", text: `Error applying template: ${e.message}` }], isError: true };
        }
      }

      // --- OMNI AUTOPILOT ALIASES ---
      // Claude Native Aliases
      case "Bash": {
        const command = (request.params.arguments as any).command;
        let resultOutput = "";
        let exitCode = 0;
        try {
          const { stdout, stderr } = await execAsync(command);
          resultOutput = String(stdout) + (stderr ? "\nSTDERR:\n" + String(stderr) : "");
        } catch (e: any) {
          resultOutput = String(e.stdout || "") + (e.stderr ? "\nSTDERR:\n" + String(e.stderr) : "") + `\nExit code: ${e.code}`;
          exitCode = e.code || 1;
        }
        const distilled = await distillText(resultOutput);
        return { 
          content: [{ type: "text", text: distilled }],
          metadata: { exitCode }
        };
      }

      case "ReadFile": {
        const filePath = (request.params.arguments as any).file_path;
        try {
          const rawContent = await fs.promises.readFile(filePath, "utf-8");
          const distilled = await distillText(rawContent);
          return { content: [{ type: "text", text: distilled }] };
        } catch (e: any) {
           return { content: [{ type: "text", text: `Error reading file: ${e.message}` }], isError: true };
        }
      }

      // Antigravity Native Aliases
      case "run_command": {
        const args = request.params.arguments as any;
        const command = args.CommandLine;
        const opts = args.Cwd ? { cwd: args.Cwd } : {};
        let resultOutput = "";
        let exitCode = 0;
        try {
          const { stdout, stderr } = await execAsync(command, opts);
          resultOutput = String(stdout) + (stderr ? "\nSTDERR:\n" + String(stderr) : "");
        } catch (e: any) {
          resultOutput = String(e.stdout || "") + (e.stderr ? "\nSTDERR:\n" + String(e.stderr) : "") + `\nExit code: ${e.code}`;
          exitCode = e.code || 1;
        }
        const distilled = await distillText(resultOutput);
        return { 
          content: [{ type: "text", text: distilled }],
          metadata: { exitCode }
        };
      }

      case "view_file": {
        const args = request.params.arguments as any;
        const filePath = args.AbsolutePath;
        const startLine = args.StartLine ? Math.max(1, args.StartLine) : 1;
        const endLine = args.EndLine ? Math.max(startLine, args.EndLine) : Infinity;

        try {
          const rawContent = await fs.promises.readFile(filePath, "utf-8");
          const lines = rawContent.split("\n");
          const targetLines = lines.slice(startLine - 1, endLine === Infinity ? undefined : endLine);
          let chunk = targetLines.join("\n");
          if (startLine > 1 || endLine < lines.length) {
             chunk = `[Showing lines ${startLine}-${Math.min(endLine, lines.length)} of ${filePath}]\n${chunk}`;
          }
          const distilled = await distillText(chunk);
          return { content: [{ type: "text", text: distilled }] };
        } catch (e: any) {
           return { content: [{ type: "text", text: `Error reading file: ${e.message}` }], isError: true };
        }
      }

      case "omni_trust": {
        try {
          if (!fs.existsSync(HOOKS_DIR)) {
            return { content: [{ type: "text", text: `No hooks directory found at ${HOOKS_DIR}. Nothing to trust.` }] };
          }

          const files = fs.readdirSync(HOOKS_DIR);
          const hashes: Record<string, string> = {};

          for (const file of files) {
            const filePath = path.join(HOOKS_DIR, file);
            if (fs.statSync(filePath).isDirectory()) continue;
            
            const content = fs.readFileSync(filePath);
            const hash = crypto.createHash('sha256').update(content).digest('hex');
            hashes[file] = hash;
          }

          const dir = path.dirname(HOOKS_SHA_FILE);
          if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
          
          fs.writeFileSync(HOOKS_SHA_FILE, JSON.stringify(hashes, null, 2));
          return { content: [{ type: "text", text: `Successfully trusted ${Object.keys(hashes).length} hooks. Hashes saved to ${HOOKS_SHA_FILE}.` }] };
        } catch (e: any) {
          return { content: [{ type: "text", text: `Error updating trusted hashes: ${e.message}` }], isError: true };
        }
      }

      case "omni_learn": {
        const args = request.params.arguments as any;
        const candidates = await discoverFilters(args.text);
        
        if (candidates.length === 0) {
          return { content: [{ type: "text", text: "No significant noise patterns discovered. Context is already high-signal!" }] };
        }

        let report = `🧠 OMNI LEARN — Discovered ${candidates.length} potential filters:\n\n`;
        for (const c of candidates) {
          report += `  - [${c.action.toUpperCase()}] ${c.name} (Conf: ${Math.round(c.confidence * 100)}%)\n`;
          report += `    Trigger: "${c.trigger}"\n`;
          report += `    Template: "${c.output}"\n\n`;
        }

        if (args.apply) {
          const targetPath = fs.existsSync(LOCAL_CONFIG_PATH) ? LOCAL_CONFIG_PATH : GLOBAL_CONFIG_PATH;
          let config: any = { rules: [], dsl_filters: [] };
          if (fs.existsSync(targetPath)) {
            config = JSON.parse(await fs.promises.readFile(targetPath, "utf-8"));
          }
          if (!config.dsl_filters) config.dsl_filters = [];
          
          let added = 0;
          for (const c of candidates) {
            // Avoid duplicates by trigger
            if (!config.dsl_filters.find((f: any) => f.trigger === c.trigger)) {
               config.dsl_filters.push({
                 name: c.name,
                 trigger: c.trigger,
                 confidence: c.confidence,
                 rules: [{ capture: c.pattern, action: c.action, as: "value_count" }],
                 output: c.output
               });
               added++;
            }
          }
          
          await fs.promises.writeFile(targetPath, JSON.stringify(config, null, 2));
          wasmInstance = null; // Reload engine
          report += `✅ Successfully applied ${added} new filters to ${targetPath}.`;
        } else {
          report += `💡 Recommendations:\n`;
          report += `To apply these filters, call 'omni_add_filter' for individual control, or re-run 'omni_learn' with { "apply": true }.`;
        }

        return { content: [{ type: "text", text: report }] };
      }

      default:
        throw new Error(`Tool not found: ${request.params.name}`);
    }
  } catch (error: any) {
    return {
      content: [{ type: "text", text: `Error: ${error.message}` }],
      isError: true,
    };
  }
});

async function main() {
  // Check for --test-integrity flag
  if (process.argv.includes("--test-integrity")) {
    await verifyHookIntegrity();
    console.log("Hook integrity check passed.");
    process.exit(0);
  }

  await verifyHookIntegrity();
  const transport = new StdioServerTransport();
  await server.connect(transport);
}

main().catch((error) => {
  console.error("Server error:", error);
  process.exit(1);
});
