import { execFile } from "node:child_process";
import type {
  ExtensionAPI,
  ExtensionContext,
  SessionStartEvent,
  BeforeAgentStartEvent,
  SessionBeforeCompactEvent,
  ToolResultEvent,
} from "@earendil-works/pi-coding-agent";

const OMNI_AGENT_ID = "pi";
const DEFAULT_OMNI_PATH = "omni";
const OMNI_TIMEOUT_MS = 10_000;
const OMNI_STDIN_LIMIT_BYTES = 16 * 1024 * 1024;

/** Patterns that identify file-mutating tools (both built-in and custom names). */
const MUTATION_TOOL_PATTERNS = ["edit", "write", "apply", "patch", "modify"] as const;

type JsonObject = Record<string, unknown>;

/** Output shape from OMNI hook subprocesses */
type OmniHookOutput = {
  hookSpecificOutput?: {
    systemPromptAddition?: string;
    updatedResponse?: string;
    additionalContext?: string;
  };
};

let omniEnabled = true;
let pendingSystemPromptAddition: string | undefined;

/** Detect whether a tool mutates files — covers both built-in and custom tool names. */
function isMutationTool(toolName: string): boolean {
  const lower = toolName.toLowerCase().trim();
  return MUTATION_TOOL_PATTERNS.some((pat) => lower.includes(pat));
}

function omniPathFromContext(ctx: ExtensionContext | undefined): string {
  const ctxObj = ctx as unknown as Record<string, unknown> | undefined;
  if (!ctxObj) return DEFAULT_OMNI_PATH;
  if (ctxObj._omniPath && typeof ctxObj._omniPath === "string") {
    return ctxObj._omniPath;
  }
  const config = ctxObj.config as JsonObject | undefined;
  if (config?.omniPath && typeof config.omniPath === "string") {
    return config.omniPath;
  }
  const extCfg = ctxObj.extensionConfig as JsonObject | undefined;
  if (extCfg?.omniPath && typeof extCfg.omniPath === "string") {
    return extCfg.omniPath;
  }
  return DEFAULT_OMNI_PATH;
}

function sessionIdFromManager(ctx: ExtensionContext): string {
  try {
    const id = ctx.sessionManager.getSessionId();
    return id || "unknown";
  } catch {
    return "unknown";
  }
}

function bytesFor(value: string): number {
  return Buffer.byteLength(value, "utf8");
}

function runOmni(
  extraArgs: string[],
  stdin: JsonObject,
  ctx: ExtensionContext,
): Promise<OmniHookOutput | undefined> {
  return new Promise((resolve) => {
    const omniPath = omniPathFromContext(ctx);
    const stdinJson = JSON.stringify(stdin);

    if (bytesFor(stdinJson) > OMNI_STDIN_LIMIT_BYTES) {
      resolve(undefined);
      return;
    }

    const args = ["--stdin", "--agent-id", OMNI_AGENT_ID, ...extraArgs];

    const child = execFile(
      omniPath,
      args,
      {
        cwd: ctx.cwd,
        timeout: OMNI_TIMEOUT_MS,
        maxBuffer: OMNI_STDIN_LIMIT_BYTES,
        env: { ...process.env },
      },
      (error, stdout) => {
        if (error) {
          resolve(undefined);
          return;
        }

        try {
          const parsed = JSON.parse(stdout) as OmniHookOutput;
          resolve(parsed);
        } catch {
          resolve(undefined);
        }
      },
    );

    if (child.stdin) {
      child.stdin.write(stdinJson);
      child.stdin.end();
    }
  });
}

async function runOmniForSessionStart(
  event: SessionStartEvent,
  ctx: ExtensionContext,
): Promise<void> {
  await runOmni(
    ["--session-start"],
    {
      hookEventName: "SessionStart",
      sessionId: sessionIdFromManager(ctx),
      reason: event.reason,
    },
    ctx,
  );
}

async function runOmniForBeforeAgentStart(
  event: BeforeAgentStartEvent,
  ctx: ExtensionContext,
): Promise<void> {
  const result = await runOmni(
    ["--before-agent-start"],
    {
      hookEventName: "BeforeAgentStart",
      sessionId: sessionIdFromManager(ctx),
      systemPromptLength: event.systemPrompt.length,
      mutationTools: Array.from(MUTATION_TOOL_PATTERNS),
    },
    ctx,
  );

  pendingSystemPromptAddition =
    result?.hookSpecificOutput?.systemPromptAddition || undefined;
}

async function runOmniForPreCompact(
  event: SessionBeforeCompactEvent,
  ctx: ExtensionContext,
): Promise<void> {
  const result = await runOmni(
    ["--pre-compact"],
    {
      hookEventName: "PreCompact",
      sessionId: sessionIdFromManager(ctx),
      compactionReason: event.customInstructions || "context_limit_reached",
    },
    ctx,
  );

  pendingSystemPromptAddition =
    result?.hookSpecificOutput?.systemPromptAddition || undefined;
}

function toolNameForOmni(toolName: string): string {
  const normalized = toolName.trim();
  const lower = normalized.toLowerCase();

  switch (lower) {
    case "bash":
    case "shell":
    case "exec":
      return "Bash";
    case "ls":
      return "LS";
    case "read":
    case "read_file":
      return "Read";
    case "grep":
    case "search":
      return "Grep";
    case "webfetch":
    case "web_fetch":
    case "fetch":
      return "WebFetch";
    case "edit":
      return "Edit";
    case "write":
      return "Write";
    default:
      return normalized;
  }
}

function textFromUnknown(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }
  if (Array.isArray(value)) {
    return value.map(textFromUnknown).filter(Boolean).join("\n");
  }
  if (typeof value === "object" && value !== null) {
    const obj = value as JsonObject;
    if (typeof obj.text === "string") {
      return obj.text;
    }
    if (typeof obj.content === "string") {
      return obj.content;
    }
    if (typeof obj.output === "string") {
      return obj.output;
    }
  }
  return "";
}

function toolResponseForOmni(event: ToolResultEvent): JsonObject {
  const text = event.content
    .map((c) => {
      if (typeof c === "string") return c;
      if (c && typeof c === "object" && "type" in c && c.type === "text") {
        return (c as { text: string }).text;
      }
      return "";
    })
    .filter(Boolean)
    .join("\n");

  return {
    toolName: toolNameForOmni(event.toolName),
    result: text,
    isError: event.isError,
  };
}

export default function omniExtension(pi: ExtensionAPI): void {
  // ── Slash commands ──

  pi.registerCommand("omni", {
    description: "Enable, disable, or check OMNI status",
    async handler(args, ctx) {
      const arg = args.trim().toLowerCase();

      if (arg === "off" || arg === "disable") {
        omniEnabled = false;
        pendingSystemPromptAddition = undefined;
        ctx.ui.notify("OMNI disabled — hooks bypassed until re-enabled", "info");
        return;
      }

      if (arg === "on" || arg === "enable") {
        omniEnabled = true;
        ctx.ui.notify("OMNI enabled — hooks will distill output normally", "info");
        return;
      }

      const status = omniEnabled ? "enabled" : "disabled";
      const skipList = Array.from(MUTATION_TOOL_PATTERNS).join(", ");
      ctx.ui.notify(
        `OMNI is ${status}\nMutation tools skipped: ${skipList}`,
        "info",
      );
    },
  });

  // ── Event handlers ──

  pi.on("session_start", async (event, ctx) => {
    if (!omniEnabled) return;
    try {
      await runOmniForSessionStart(event, ctx);
    } catch {
      // OMNI fails silently — never crash the host
    }
  });

  pi.on("before_agent_start", async (event, ctx) => {
    if (!omniEnabled) return;
    try {
      await runOmniForBeforeAgentStart(event, ctx);
    } catch {
      // OMNI fails silently — never crash the host
    }
  });

  pi.on("session_before_compact", async (event, ctx) => {
    if (!omniEnabled) return;
    try {
      await runOmniForPreCompact(event, ctx);
    } catch {
      // OMNI fails silently — never crash the host
    }
  });

  pi.on("tool_result", async (event, ctx) => {
    if (!omniEnabled) return;

    const toolName = event.toolName;

    // Skip file-mutating tools — OMNI only distills non-mutating tool results.
    // Per PDD canvas: "tool_result calls omni --post-hook for non-mutating tools"
    if (isMutationTool(toolName)) {
      return;
    }

    try {
      await runOmni(
        ["--tool-result"],
        {
          hookEventName: "ToolResult",
          sessionId: sessionIdFromManager(ctx),
          toolName: toolNameForOmni(toolName),
          toolResponse: toolResponseForOmni(event),
          isError: event.isError,
        },
        ctx,
      );
    } catch {
      // OMNI fails silently — never crash the host
    }
  });
}
