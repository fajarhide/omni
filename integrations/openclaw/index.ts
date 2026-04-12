import { execFile } from "child_process";
import { Type } from "@sinclair/typebox";
import { definePluginEntry } from "openclaw/plugin-sdk/plugin-entry";
import type { AnyAgentTool, OpenClawPluginApi, OpenClawPluginToolFactory } from "./runtime-api.js";

const DANGEROUS_ENV_VARS = [
  "BASH_ENV", "ENV", "ZDOTDIR", "BASH_PROFILE", "PROMPT_COMMAND", "IFS",
  "NODE_OPTIONS", "PYTHONSTARTUP", "RUBYOPT", "JAVA_TOOL_OPTIONS",
  "LD_PRELOAD", "LD_LIBRARY_PATH", "DYLD_INSERT_LIBRARIES", "DYLD_FORCE_FLAT_NAMESPACE",
  "PYTHONPATH", "PYTHONHOME", "RUBYLIB",
  "GIT_ASKPASS", "GIT_EXEC_PATH", "GIT_TEMPLATE_DIR"
] as const;

function sanitizeEnv(env: Record<string, string | undefined>): Record<string, string | undefined> {
  const sanitized = { ...process.env as Record<string, string> };
  for (const v of DANGEROUS_ENV_VARS) {
    delete sanitized[v];
  }
  return sanitized;
}

async function runOmni(bin: string, args: string[]): Promise<{ stdout: string; stderr: string; code: number }> {
  return new Promise((resolve) => {
    const sanitizedEnv = sanitizeEnv(process.env as Record<string, string>);
    execFile(bin, args, { shell: false, env: sanitizedEnv }, (error, stdout, stderr) => {
      resolve({
        stdout: stdout || "",
        stderr: stderr || "",
        code: error ? (error as NodeJS.ErrnoException).code ?? 1 : 0
      });
    });
  });
}

const OmniCmdParams = Type.Object({
  command: Type.String({ description: "The terminal command to execute (e.g. 'npm install' or 'git diff')" })
});

type PluginConfig = { omniPath?: string };

function createOmniCmdTool(api: OpenClawPluginApi): AnyAgentTool {
  return {
    name: "omni_cmd",
    label: "OMNI Command",
    description: "Execute terminal tools (git, npm, cargo, docker, etc.) through OMNI's local semantic distillation engine to save 80-90% of token costs.",
    parameters: OmniCmdParams,
    async execute(_toolCallId: string, params: Record<string, unknown>) {
      const command = params.command as string;
      const config = (api.pluginConfig ?? {}) as PluginConfig;
      const omniPath = config.omniPath || "omni";

      try {
        const { stdout, stderr, code } = await runOmni(omniPath, ["exec", "--", command]);
        let result = stdout || "";
        if (stderr && stderr.trim()) {
          result += `\n[stderr]\n${stderr}`;
        }
        return {
          content: [{ type: "text" as const, text: result || (code === 0 ? "(Command completed)" : "(Command failed)") }],
          details: { exitCode: code }
        };
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        return {
          content: [{ type: "text" as const, text: `Error running OMNI: ${message}` }],
          details: { error: true }
        };
      }
    }
  };
}

export default definePluginEntry({
  id: "omni-signal-engine",
  name: "OMNI Semantic Signal Engine",
  description: "Local-only semantic context filtering for OpenClaw using OMNI. Saves tokens by distilling shell output.",
  configSchema: Type.Object({
    omniPath: Type.Optional(Type.String({
      description: "Path to the omni binary (defaults to 'omni' in PATH)",
      default: "omni"
    }))
  }),
  register(api: OpenClawPluginApi) {
    if (api.registrationMode !== "full") {
      return;
    }
    api.registerTool(createOmniCmdTool(api) as OpenClawPluginToolFactory, { optional: true });
  }
});
