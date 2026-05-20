import { execFile } from "node:child_process";
import type {
  ExtensionAPI,
  SessionStartEvent,
  BeforeAgentStartEvent,
  SessionBeforeCompactEvent,
  ToolResultEvent,
} from "@earendil-works/pi-coding-agent";

const OMNI_AGENT_ID = "pi";
const DEFAULT_OMNI_PATH = "omni";
const OMNI_TIMEOUT_MS = 10_000;
const OMNI_STDIN_LIMIT_BYTES = 16 * 1024 * 1024;

/** Tools whose results are NOT sent to OMNI (mutation ops handled separately). */
const EXCLUDE_TOOL_NAMES = new Set(["edit", "write"]);

type JsonObject = Record<string, unknown>;

type OmniHookOutput = {
  hookSpecificOutput?: {
    systemPromptAddition?: string;
    updatedResponse?: string;
    additionalContext?: string;
  };
};

/** State shared across hooks — toggled by /omni command */
let omniEnabled = true;
let pendingSystemPromptAddition: string | undefined;

function omniPathOrDefault(config?: unknown): string {
  if (typeof config === "object" && config !== null) {
    const obj = config as Record<string, unknown>;
    if (typeof obj._omniPath === "string") return obj._omniPath;
    const cfg = obj.config as JsonObject | undefined;
    if (cfg?.omniPath && typeof cfg.omniPath === "string") return cfg.omniPath as string;
    const ext = obj.extensionConfig as JsonObject | undefined;
    if (ext?.omniPath && typeof ext.omniPath === "string") return ext.omniPath as string;
  }
  return DEFAULT_OMNI_PATH;
}

function sessionId(ctx: unknown): string {
  try {
    const obj = ctx as Record<string, unknown> | undefined;
    return (obj?.sessionManager as { getSessionId?: () => string | undefined })
      ?.getSessionId?.() || "unknown";
  } catch {
    return "unknown";
  }
}

/**
 * Call the `omni` binary.
 * JSON goes in via stdin; OMNI_AGENT_ID is injected as an env var.
 * The extension flags used by the Pi runtime have NO relation to OMNI CLI flags —
 * `--stdin` and `--agent-id` do NOT exist on the OMNI binary and would cause
 * "unknown command" failures that get silently swallowed.
 */
function runOmni(
  omniFlag: string,
  payload: JsonObject,
  cwd: string,
  ctxForConfig?: unknown,
): Promise<OmniHookOutput | undefined> {
  return new Promise((resolve) => {
    const omni = omniPathOrDefault(ctxForConfig);
    const body = JSON.stringify(payload);

    if (Buffer.byteLength(body, "utf8") > OMNI_STDIN_LIMIT_BYTES) {
      resolve(undefined);
      return;
    }

    const child = execFile(
      omni,
      [omniFlag],
      {
        cwd,
        timeout: OMNI_TIMEOUT_MS,
        maxBuffer: OMNI_STDIN_LIMIT_BYTES,
        env: { ...process.env, OMNI_AGENT_ID },
      },
      (error, stdout) => {
        if (error) {
          resolve(undefined);
          return;
        }
        try {
          resolve(JSON.parse(stdout) as OmniHookOutput);
        } catch {
          resolve(undefined);
        }
      },
    );

    if (child.stdin) {
      child.stdin.end(body);
    }
  });
}

// ── Helpers for OMNI payloads ──

function toolNameForOmni(name: string): string {
  const n = name.toLowerCase().trim();
  switch (n) {
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
    default:
      return name.trim() || name;
  }
}

function extractText(value: unknown): string {
  if (typeof value === "string") return value;
  if (Array.isArray(value)) return value.map(extractText).filter(Boolean).join("\n");
  if (typeof value === "object" && value !== null) {
    const o = value as JsonObject;
    return (
      (typeof o.text === "string" && o.text) ||
      (typeof o.content === "string" && o.content) ||
      (typeof o.output === "string" && o.output) ||
      ""
    );
  }
  return "";
}

function toolResultPayload(event: ToolResultEvent): JsonObject {
  const text = (event as { content?: unknown[] }).content
    ?.map((c) => {
      if (typeof c === "string") return c;
      if (typeof c === "object" && c !== null && "type" in c && (c as { type: string }).type === "text") {
return (c as unknown as { text: string }).text;
      }
      return "";
    })
    .filter(Boolean)
    .join("\n") ?? extractText((event as { output?: unknown }).output);

  return {
    toolName: toolNameForOmni((event as { toolName: string }).toolName),
    result: text,
    isError: !!(event as { isError?: boolean }).isError,
  };
}

// ── Extension entry point ──

export default function omniExtension(pi: ExtensionAPI): void {
  // ── Slash command ──

  pi.registerCommand("omni", {
    description: "Enable, disable, or check OMNI status",
    async handler(argStr, ctx) {
      const arg = argStr.trim().toLowerCase();
      if (arg === "off" || arg === "disable") {
        omniEnabled = false;
        pendingSystemPromptAddition = undefined;
        ctx.ui.notify("OMNI disabled", "info");
        return;
      }
      if (arg === "on" || arg === "enable") {
        omniEnabled = true;
        ctx.ui.notify("OMNI enabled", "info");
        return;
      }
      ctx.ui.notify(
        omniEnabled ? "OMNI is enabled" : "OMNI is disabled",
        "info",
      );
    },
  });

  // ── Hook: session_start ──

  pi.on("session_start", async (event, ctx) => {
    if (!omniEnabled) return;
    runOmni(
      "--session-start",
      {
        hookEventName: "SessionStart",
        sessionId: sessionId(ctx),
        workingDirectory: ctx.cwd,
        reason: event.reason,
      },
      ctx.cwd,
      ctx,
    ).catch(() => {
      /* fail-open */
    });
  });

  // ── Hook: before_agent_start ──

  pi.on("before_agent_start", async (event, ctx) => {
    if (!omniEnabled) return;
    try {
      const out = await runOmni(
        "--before-agent-start",
        {
          hookEventName: "BeforeAgentStart",
          sessionId: sessionId(ctx),
          workingDirectory: ctx.cwd,
          systemPromptLength: event.systemPrompt.length,
          mutationTools: Array.from(EXCLUDE_TOOL_NAMES),
        },
        ctx.cwd,
        ctx,
      );
      pendingSystemPromptAddition =
        out?.hookSpecificOutput?.systemPromptAddition ?? undefined;
    } catch {
      /* fail-open */
    }
  });

  // ── Hook: session_before_compact ──

  pi.on("session_before_compact", async (event, ctx) => {
    if (!omniEnabled) return;
    try {
      const out = await runOmni(
        "--pre-compact",
        {
          hookEventName: "PreCompact",
          sessionId: sessionId(ctx),
          compactionReason:
            (event as { customInstructions?: string }).customInstructions ||
            "context_limit_reached",
        },
        ctx.cwd,
        ctx,
      );
      pendingSystemPromptAddition =
        out?.hookSpecificOutput?.systemPromptAddition ?? undefined;
    } catch {
      /* fail-open */
    }
  });

  // ── Hook: tool_result (non-mutating only) ──

  pi.on("tool_result", async (event, ctx) => {
    if (!omniEnabled) return;
    const name = (event as { toolName: string }).toolName;
    if (EXCLUDE_TOOL_NAMES.has(name)) return;

    runOmni(
      "--post-hook",
      {
        hookEventName: "ToolResult",
        sessionId: sessionId(ctx),
        toolName: toolNameForOmni(name),
        toolResponse: toolResultPayload(event),
        isError: !!(event as { isError?: boolean }).isError,
      },
      ctx.cwd,
      ctx,
    ).catch(() => {
      /* fail-open */
    });
  });
}
