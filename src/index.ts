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

// Sandbox Env Denylist: block dangerous env vars from child processes
const ENV_DENYLIST = new Set([
  // Shell injection vectors
  'BASH_ENV', 'ENV', 'ZDOTDIR', 'BASH_PROFILE', 'BASH_LOGIN',
  'BASH_LOGOUT', 'PROFILE', 'INPUTRC', 'HISTFILE',
  // Node.js
  'NODE_OPTIONS', 'NODE_EXTRA_CA_CERTS', 'NODE_PATH',
  'NODE_REDIRECT_WARNINGS', 'NODE_REPL_HISTORY',
  // Python
  'PYTHONSTARTUP', 'PYTHONPATH', 'PYTHONHOME', 'PYTHONWARNINGS',
  'PYTHONDONTWRITEBYTECODE', 'PYTHONHASHSEED',
  // Ruby
  'RUBYOPT', 'RUBYLIB', 'GEM_PATH', 'GEM_HOME', 'BUNDLE_GEMFILE',
  // Perl
  'PERL5OPT', 'PERL5LIB', 'PERLLIB',
  // Java
  'JAVA_TOOL_OPTIONS', '_JAVA_OPTIONS', 'JDK_JAVA_OPTIONS',
  'JAVA_HOME', 'CLASSPATH',
  // Dynamic linker hijacking (Linux)
  'LD_PRELOAD', 'LD_LIBRARY_PATH', 'LD_AUDIT',
  'LD_PROFILE', 'LD_SHOW_AUXV', 'LD_DEBUG',
  // Dynamic linker hijacking (macOS)
  'DYLD_INSERT_LIBRARIES', 'DYLD_FORCE_FLAT_NAMESPACE',
  'DYLD_LIBRARY_PATH', 'DYLD_FRAMEWORK_PATH', 'DYLD_FALLBACK_LIBRARY_PATH',
  // Curl / HTTP
  'CURL_CA_BUNDLE', 'SSL_CERT_FILE', 'SSL_CERT_DIR',
  'REQUESTS_CA_BUNDLE', 'HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY',
  'NO_PROXY', 'http_proxy', 'https_proxy',
  // Git
  'GIT_EXEC_PATH', 'GIT_TEMPLATE_DIR', 'GIT_CONFIG_GLOBAL',
  'GIT_ASKPASS', 'GIT_SSH_COMMAND',
  // Misc
  'EDITOR', 'VISUAL', 'BROWSER', 'PAGER',
  'PROMPT_COMMAND', 'PS1', 'PS2', 'PS4',
  'IFS', 'CDPATH', 'GLOBIGNORE', 'MAIL', 'MAILPATH',
]);

function sanitizeEnv(env: NodeJS.ProcessEnv): NodeJS.ProcessEnv {
  const clean = { ...env };
  for (const key of ENV_DENYLIST) delete clean[key];
  return clean;
}
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

// Security: Project Trust Boundary
const TRUSTED_PROJECTS_PATH = path.join(os.homedir(), ".omni", "trusted-projects.json");

interface TrustedProject {
  path: string;
  configHash: string;
  trustedAt: string;
}

function loadTrustedProjects(): TrustedProject[] {
  try {
    if (fs.existsSync(TRUSTED_PROJECTS_PATH)) {
      return JSON.parse(fs.readFileSync(TRUSTED_PROJECTS_PATH, "utf-8"));
    }
  } catch (e) {
    console.error("Error reading trusted-projects.json:", e);
  }
  return [];
}

function saveTrustedProjects(projects: TrustedProject[]): void {
  const dir = path.dirname(TRUSTED_PROJECTS_PATH);
  if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(TRUSTED_PROJECTS_PATH, JSON.stringify(projects, null, 2));
}

function hashFileContent(filePath: string): string {
  const content = fs.readFileSync(filePath);
  return crypto.createHash('sha256').update(content).digest('hex');
}

function isProjectTrusted(projectPath: string): { trusted: boolean; reason?: string } {
  const configPath = path.join(projectPath, "omni_config.json");
  if (!fs.existsSync(configPath)) return { trusted: false, reason: "no_config" };

  const projects = loadTrustedProjects();
  const normalizedPath = path.resolve(projectPath);
  const entry = projects.find(p => path.resolve(p.path) === normalizedPath);

  if (!entry) {
    return { trusted: false, reason: "not_trusted" };
  }

  // Verify config hasn't been tampered with since trust was granted
  const currentHash = hashFileContent(configPath);
  if (currentHash !== entry.configHash) {
    return { trusted: false, reason: "config_modified" };
  }

  return { trusted: true };
}

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

  // 2. Merge Local Config (requires explicit trust)
  if (fs.existsSync(LOCAL_CONFIG_PATH)) {
    const trustCheck = isProjectTrusted(process.cwd());
    if (!trustCheck.trusted) {
      const reasons: Record<string, string> = {
        not_trusted: "Local config not trusted. Run omni_trust to review and trust.",
        config_modified: "Local config modified since last trust. Run omni_trust to re-verify.",
        no_config: "No local config found."
      };
      console.error(`⚠ ${reasons[trustCheck.reason || "not_trusted"]} Skipping local config.`);
    } else {
      try {
        const localRaw = fs.readFileSync(LOCAL_CONFIG_PATH, "utf-8");
        const localConfig = JSON.parse(localRaw);
        if (localConfig.rules) config.rules.push(...localConfig.rules);
        if (localConfig.dsl_filters) config.dsl_filters.push(...localConfig.dsl_filters);
      } catch (e) {
        console.error("Error parsing local config:", e);
      }
    }
  }

  return config;
}

type TemplateBundle = {
  rules?: any[];
  dsl_filters?: any[];
};

const CODEX_ADVANCED_FILTERS = [
  {
    name: "codex-tsc-summary",
    trigger: "error TS",
    confidence: 0.98,
    rules: [
      { capture: "{location}: error TS{code}: {message}", action: "keep" },
      { capture: "{ts_location}: error TS{ts_code}: {ts_message}", action: "count", as: "diagnostics" }
    ],
    output: "tsc: {diagnostics} diagnostics | last TS{code}: {message}"
  },
  {
    name: "codex-eslint-summary-plural",
    trigger: "problems (",
    confidence: 0.97,
    rules: [
      { capture: "✖ {problems} problems ({errors} errors, {warnings} warnings)", action: "keep" }
    ],
    output: "eslint: {problems} problems | {errors} errors | {warnings} warnings"
  },
  {
    name: "codex-eslint-summary-singular",
    trigger: "problem (",
    confidence: 0.97,
    rules: [
      { capture: "✖ {problems} problem ({errors} error, {warnings} warning)", action: "keep" }
    ],
    output: "eslint: {problems} problem | {errors} error | {warnings} warning"
  },
  {
    name: "codex-jest-summary-fail",
    trigger: "failed, ",
    confidence: 0.96,
    rules: [
      { capture: "Tests:       {failed} failed, {passed} passed, {total} total", action: "keep" }
    ],
    output: "jest: {passed} passed | {failed} failed | {total} total"
  },
  {
    name: "codex-jest-summary-pass",
    trigger: "Tests:",
    confidence: 0.95,
    rules: [
      { capture: "Tests:       {passed} passed, {total} total", action: "keep" }
    ],
    output: "jest: {passed} passed | {total} total"
  },
  {
    name: "codex-vitest-summary",
    trigger: "Test Files",
    confidence: 0.95,
    rules: [
      { capture: "Test Files {files} passed", action: "keep" },
      { capture: "Tests {tests} passed", action: "keep" }
    ],
    output: "vitest: {files} files passed | {tests} tests passed"
  }
];

const PYTEST_ADVANCED_FILTERS = [
  {
    name: "pytest-summary-fail",
    trigger: " failed, ",
    confidence: 0.95,
    rules: [
      { capture: "{failed} failed, {passed} passed in {duration}", action: "keep" }
    ],
    output: "pytest: {passed} passed | {failed} failed | {duration}"
  },
  {
    name: "pytest-summary-pass",
    trigger: " passed in ",
    confidence: 0.94,
    rules: [
      { capture: "{passed} passed in {duration}", action: "keep" }
    ],
    output: "pytest: {passed} passed | {duration}"
  }
];

const RUFF_ADVANCED_FILTERS = [
  {
    name: "ruff-summary-pass",
    trigger: "All checks passed!",
    confidence: 0.99,
    rules: [],
    output: "ruff: all checks passed"
  },
  {
    name: "ruff-summary-errors-plural",
    trigger: "Found ",
    confidence: 0.96,
    rules: [
      { capture: "Found {errors} errors.", action: "keep" }
    ],
    output: "ruff: {errors} errors"
  },
  {
    name: "ruff-summary-errors-singular",
    trigger: "Found 1 error.",
    confidence: 0.96,
    rules: [
      { capture: "Found {error} error.", action: "keep" }
    ],
    output: "ruff: {error} error"
  }
];

const CARGO_TEST_ADVANCED_FILTERS = [
  {
    name: "cargo-test-summary-pass",
    trigger: "test result: ok.",
    confidence: 0.97,
    rules: [
      { capture: "test result: ok. {passed} passed; {failed} failed; {ignored} ignored; {measured} measured; {filtered} filtered out; finished in {duration}", action: "keep" }
    ],
    output: "cargo test: {passed} passed | {failed} failed | {duration}"
  },
  {
    name: "cargo-test-summary-fail",
    trigger: "test result: FAILED.",
    confidence: 0.97,
    rules: [
      { capture: "test result: FAILED. {passed} passed; {failed} failed; {ignored} ignored; {measured} measured; {filtered} filtered out; finished in {duration}", action: "keep" }
    ],
    output: "cargo test: {passed} passed | {failed} failed | {duration}"
  }
];

const PNPM_ADVANCED_FILTERS = [
  {
    name: "pnpm-install-summary",
    trigger: "Progress: resolved",
    confidence: 0.93,
    rules: [
      { capture: "Progress: resolved {resolved}, reused {reused}, downloaded {downloaded}, added {added}, done", action: "keep" },
      { capture: "Done in {duration}", action: "keep" }
    ],
    output: "pnpm: resolved {resolved} | reused {reused} | downloaded {downloaded} | added {added} | {duration}"
  }
];

const ZIG_ADVANCED_FILTERS = [
  {
    name: "zig-test-summary-pass",
    trigger: "tests passed.",
    confidence: 0.92,
    rules: [
      { capture: "All {passed} tests passed.", action: "keep" }
    ],
    output: "zig test: {passed} passed"
  }
];

const GO_TEST_ADVANCED_FILTERS = [
  {
    name: "go-test-summary-pass",
    trigger: "ok\t",
    confidence: 0.9,
    rules: [
      { capture: "ok\t{pkg}\t{duration}", action: "keep" },
      { capture: "ok\t{counted_pkg}\t{counted_duration}", action: "count", as: "passed_packages" }
    ],
    output: "go test: {passed_packages} packages passed | last {pkg} | {duration}"
  },
  {
    name: "go-test-summary-fail",
    trigger: "FAIL\t",
    confidence: 0.9,
    rules: [
      { capture: "FAIL\t{failed_pkg}\t{failed_duration}", action: "keep" },
      { capture: "FAIL\t{counted_failed_pkg}\t{counted_failed_duration}", action: "count", as: "failed_packages" }
    ],
    output: "go test: {failed_packages} packages failed | last {failed_pkg} | {failed_duration}"
  }
];

const TEMPLATES: Record<string, TemplateBundle> = {
  "kubernetes": {
    rules: [
      { name: "k8s_uid", match: "uid:", action: "mask" },
      { name: "k8s_managed_fields", match: "managedFields:", action: "remove" }
    ]
  },
  "terraform": {
    rules: [
      { name: "tf_refresh", match: "Refreshing state...", action: "remove" },
      { name: "tf_no_changes", match: "No changes. Your infrastructure matches the configuration.", action: "mask" }
    ]
  },
  "node-verbose": {
    rules: [
      { name: "npm_notice", match: "npm notice", action: "remove" },
      { name: "node_modules_path", match: "node_modules/", action: "mask" }
    ]
  },
  "docker-layers": {
    rules: [
      { name: "docker_hash", match: "sha256:", action: "mask" }
    ]
  },
  "security-audit": {
    rules: [
      { name: "ip_mask", match: "192.168.", action: "mask" },
      { name: "password_remove", match: "password:", action: "remove" },
      { name: "key_mask", match: "PRIVATE KEY", action: "mask" }
    ]
  },
  "aws-cloud": {
    rules: [
      { name: "aws_request_id", match: "RequestId:", action: "remove" },
      { name: "aws_arn_mask", match: "arn:aws:", action: "mask" }
    ]
  },
  "codex-advanced": {
    dsl_filters: CODEX_ADVANCED_FILTERS
  },
  "pytest-advanced": {
    dsl_filters: PYTEST_ADVANCED_FILTERS
  },
  "ruff-advanced": {
    dsl_filters: RUFF_ADVANCED_FILTERS
  },
  "cargo-test-advanced": {
    dsl_filters: CARGO_TEST_ADVANCED_FILTERS
  },
  "pnpm-advanced": {
    dsl_filters: PNPM_ADVANCED_FILTERS
  },
  "zig-advanced": {
    dsl_filters: ZIG_ADVANCED_FILTERS
  },
  "go-test-advanced": {
    dsl_filters: GO_TEST_ADVANCED_FILTERS
  },
  "codex-polyglot": {
    dsl_filters: [
      ...CODEX_ADVANCED_FILTERS,
      ...PYTEST_ADVANCED_FILTERS,
      ...RUFF_ADVANCED_FILTERS,
      ...CARGO_TEST_ADVANCED_FILTERS,
      ...PNPM_ADVANCED_FILTERS,
      ...ZIG_ADVANCED_FILTERS,
      ...GO_TEST_ADVANCED_FILTERS
    ]
  }
};

let wasmInstance: WebAssembly.Instance | null = null;
let wasi: WASI | null = null;

function mergeTemplate(config: any, template: TemplateBundle) {
  if (!config.rules) config.rules = [];
  if (!config.dsl_filters) config.dsl_filters = [];

  for (const rule of template.rules ?? []) {
    if (!config.rules.find((existing: any) => existing.name === rule.name)) {
      config.rules.push(rule);
    }
  }

  for (const filter of template.dsl_filters ?? []) {
    if (!config.dsl_filters.find((existing: any) => existing.name === filter.name)) {
      config.dsl_filters.push(filter);
    }
  }
}

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
              enum: ["kubernetes", "terraform", "node-verbose", "docker-layers", "security-audit", "aws-cloud", "codex-advanced", "pytest-advanced", "ruff-advanced", "cargo-test-advanced", "pnpm-advanced", "zig-advanced", "go-test-advanced", "codex-polyglot"],
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
        name: "omni_trust_hooks",
        description: "Verify and store SHA-256 hashes of hook scripts in ~/.omni/hooks. Run this after you manually inspect and approve new or modified hooks.",
        inputSchema: {
          type: "object",
          properties: {},
          required: [],
        },
      },
      {
        name: "omni_trust",
        description: "Review and trust a project's local omni_config.json. OMNI will not load local configs until explicitly trusted. Shows config contents and SHA-256 hash before trusting. Re-run after modifying the config.",
        inputSchema: {
          type: "object",
          properties: {
            projectPath: { type: "string", description: "Path to the project directory (defaults to current working directory)" }
          },
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
        const opts: any = { env: sanitizeEnv(process.env) };
        if (args.cwd) opts.cwd = args.cwd;
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
            const { stdout, stderr } = await execFileAsync('grep', [...flags, query, targetPath], { env: sanitizeEnv(process.env) });
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
             const { stdout } = await execFileAsync('find', [dir, '-name', pattern], { env: sanitizeEnv(process.env) });
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
        const template = TEMPLATES[templateName];
        if (!template) throw new Error(`Template not found: ${templateName}`);

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
          
          mergeTemplate(config, template);

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
          const { stdout, stderr } = await execAsync(command, { env: sanitizeEnv(process.env) });
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
        const opts: any = { env: sanitizeEnv(process.env) };
        if (args.Cwd) opts.cwd = args.Cwd;
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

      case "omni_trust_hooks": {
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

      case "omni_trust": {
        const args = request.params.arguments as any;
        const projectDir = path.resolve(args.projectPath || process.cwd());
        const configPath = path.join(projectDir, "omni_config.json");

        try {
          if (!fs.existsSync(configPath)) {
            return { content: [{ type: "text", text: `No omni_config.json found in ${projectDir}. Nothing to trust.` }] };
          }

          // Read and display the config for review
          const rawConfig = fs.readFileSync(configPath, "utf-8");
          const configHash = hashFileContent(configPath);
          let parsedConfig: any;
          try {
            parsedConfig = JSON.parse(rawConfig);
          } catch {
            return { content: [{ type: "text", text: `Error: ${configPath} is not valid JSON.` }], isError: true };
          }

          const ruleCount = (parsedConfig.rules?.length || 0) + (parsedConfig.dsl_filters?.length || 0);

          // Check existing trust status
          const projects = loadTrustedProjects();
          const existingIdx = projects.findIndex(p => path.resolve(p.path) === projectDir);
          const wasAlreadyTrusted = existingIdx !== -1;

          // Update or add trust entry
          const entry: TrustedProject = {
            path: projectDir,
            configHash,
            trustedAt: new Date().toISOString()
          };

          if (existingIdx !== -1) {
            projects[existingIdx] = entry;
          } else {
            projects.push(entry);
          }

          saveTrustedProjects(projects);

          // Force Wasm engine reload to pick up newly trusted config
          wasmInstance = null;

          const status = wasAlreadyTrusted ? "🔄 Re-trusted" : "✅ Trusted";
          const report =
            `${status} project: ${projectDir}\n` +
            `Config: ${configPath}\n` +
            `SHA-256: ${configHash}\n` +
            `Rules: ${ruleCount} (${parsedConfig.rules?.length || 0} rules, ${parsedConfig.dsl_filters?.length || 0} DSL filters)\n` +
            `\n--- Config Contents ---\n${JSON.stringify(parsedConfig, null, 2)}`;

          return { content: [{ type: "text", text: report }] };
        } catch (e: any) {
          return { content: [{ type: "text", text: `Error trusting project: ${e.message}` }], isError: true };
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
