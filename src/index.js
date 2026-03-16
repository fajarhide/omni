"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
const index_js_1 = require("@modelcontextprotocol/sdk/server/index.js");
const stdio_js_1 = require("@modelcontextprotocol/sdk/server/stdio.js");
const types_js_1 = require("@modelcontextprotocol/sdk/types.js");
const child_process_1 = require("child_process");
const path_1 = __importDefault(require("path"));
const server = new index_js_1.Server({
    name: "omni-server",
    version: "0.3.9",
}, {
    capabilities: {
        tools: {},
    },
});
const ZIG_BINARY_PATH = path_1.default.join(process.cwd(), "core", "zig-out", "bin", "omni");
server.setRequestHandler(types_js_1.ListToolsRequestSchema, async () => {
    return {
        tools: [
            {
                name: "omni_compress",
                description: "Compress a string to save LLM tokens using Zig-powered OMNI engine.",
                inputSchema: {
                    type: "object",
                    properties: {
                        text: {
                            type: "string",
                            description: "The raw text to compress",
                        },
                    },
                    required: ["text"],
                },
            },
        ],
    };
});
server.setRequestHandler(types_js_1.CallToolRequestSchema, async (request) => {
    if (request.params.name === "omni_compress") {
        const text = request.params.arguments.text;
        try {
            // Pipe input to the Zig binary and capture output
            const output = (0, child_process_1.execSync)(ZIG_BINARY_PATH, {
                input: text,
                encoding: "utf-8",
            });
            return {
                content: [
                    {
                        type: "text",
                        text: output.trim(),
                    },
                ],
            };
        }
        catch (error) {
            return {
                content: [
                    {
                        type: "text",
                        text: `Error calling OMNI engine: ${error.message}`,
                    },
                ],
                isError: true,
            };
        }
    }
    throw new Error("Tool not found");
});
async function main() {
    const transport = new stdio_js_1.StdioServerTransport();
    await server.connect(transport);
    console.error("OMNI MCP Server running on stdio");
}
main().catch((error) => {
    console.error("Server error:", error);
    process.exit(1);
});
//# sourceMappingURL=index.js.map