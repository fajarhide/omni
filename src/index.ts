import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import fs from "fs";
import path from "path";
import { promisify } from "util";
import { exec } from "child_process";
import { fileURLToPath } from "url";
import { WASI } from "wasi";
import { LRUCache } from "./cache.js";
import os from "os";

const execAsync = promisify(exec);
const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Parse Agent Argument (--agent=claude-code)
let currentAgent = "unknown";
for (const arg of process.argv) {
  if (arg.startsWith("--agent=")) {
    currentAgent = arg.split("=")[1] || "unknown";
  }
}

// Telemetry Logic
const TELEMETRY_FILE = path.join(os.homedir(), ".omni", "telemetry.csv");

async function logTelemetry(inputLen: number, outputLen: number, ms: number) {
  try {
    const timestamp = Math.floor(Date.now() / 1000);
    const line = `${timestamp},${currentAgent},${inputLen},${outputLen},${Math.round(ms)}\n`;
    
    // Ensure .omni directory exists
    const dir = path.dirname(TELEMETRY_FILE);
    if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });

    await fs.promises.appendFile(TELEMETRY_FILE, line);
  } catch (e) {
    // Ignore logging errors to remain transparent
  }
}

const server = new Server(
  {
    name: "omni-server",
    version: "0.3.9",
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

const WASM_PATH = path.join(__dirname, "..", "core", "omni-wasm.wasm");
let wasmInstance: WebAssembly.Instance | null = null;
let wasi: WASI | null = null;

async function getWasmInstance() {
  if (wasmInstance) return wasmInstance;

  wasi = new WASI({
    version: "preview1",
    args: [],
    env: process.env,
    preopens: {
      ".": path.join(__dirname, "..", "core"),
    },
  });

  const wasmBuffer = fs.readFileSync(WASM_PATH);
  const { instance } = await WebAssembly.instantiate(wasmBuffer, {
    wasi_snapshot_preview1: wasi.wasiImport,
  });

  wasmInstance = instance;
  const exports = wasmInstance.exports as any;
  if (!exports.init_engine()) {
    console.error("Failed to initialize OMNI engine in Wasm");
  }

  return wasmInstance;
}

// Helper: Distill text via OMNI Wasm engine
async function distillText(text: string): Promise<string> {
  const startTime = performance.now();
  const cached = cache.get(text);
  if (cached) {
    await logTelemetry(text.length, cached.length, performance.now() - startTime);
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

  const memView = new Uint8Array(memory.buffer);
  memView.set(inputBytes, inputPtr);

  // Call compress: returns struct { ptr, len }
  const resultRaw = exports.compress(inputPtr, inputBytes.length);
  const resultPtr = Number(BigInt(resultRaw) & 0xFFFFFFFFn);
  const resultLen = Number(BigInt(resultRaw) >> 32n);

  const outputBytes = new Uint8Array(memory.buffer, resultPtr, resultLen);
  const output = decoder.decode(outputBytes);

  exports.free(inputPtr, inputBytes.length);
  exports.free(resultPtr, resultLen);

  const trimmed = output.trim();
  cache.set(text, trimmed);
  
  const elapsed = performance.now() - startTime;
  await logTelemetry(text.length, trimmed.length, elapsed);
  
  return trimmed;
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
        
        try {
          const { stdout, stderr } = await execAsync(args.command, opts);
          resultOutput = String(stdout) + (stderr ? "\nSTDERR:\n" + String(stderr) : "");
        } catch (e: any) {
          resultOutput = String(e.stdout || "") + (e.stderr ? "\nSTDERR:\n" + String(e.stderr) : "") + `\nExit code: ${e.code}`;
        }
        
        const distilled = await distillText(resultOutput);
        return { content: [{ type: "text", text: distilled }] };
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
  const transport = new StdioServerTransport();
  await server.connect(transport);
}

main().catch((error) => {
  console.error("Server error:", error);
  process.exit(1);
});
