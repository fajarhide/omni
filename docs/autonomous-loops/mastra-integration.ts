/**
 * OMNI Mastra/Vercel AI SDK Integration
 *
 * TypeScript helper to consume `omni handoff --json` in a Mastra workflow.
 *
 * Usage:
 *   import { OmniClient, shouldContinueLoop } from './mastra-integration';
 *
 *   const omni = new OmniClient();
 *   while (shouldContinueLoop(await omni.getStatus())) {
 *     // ... run agent step ...
 *   }
 */

import { execSync } from "child_process";

export interface OmniHandoff {
  schema_version: number;
  session_id: string;
  context_pressure: string;
  estimated_tokens: number;
  recommendation: {
    action: "CONTINUE" | "COMPACT_OR_ESCALATE" | "ESCALATE" | "DONE";
    reason: string;
  };
  loop_context: {
    loop_id: string;
    iteration: number;
    budget_tokens: number;
    budget_used: number;
    goal: string;
  };
  engrams: Array<{
    type: string;
    content: string;
    timestamp: number;
  }>;
}

export interface OmniStats {
  commands_processed: number;
  tokens_saved: number;
  avg_latency_ms: number;
  compression_ratio: number;
}

export class OmniClient {
  private binaryPath: string;

  constructor(binaryPath = "omni") {
    this.binaryPath = binaryPath;
  }

  /**
   * Get current session handoff state
   */
  getStatus(): OmniHandoff {
    const output = execSync(`${this.binaryPath} handoff --json`, {
      encoding: "utf-8",
      timeout: 5000,
    });
    return JSON.parse(output);
  }

  /**
   * Get token savings statistics
   */
  getStats(): OmniStats {
    const output = execSync(`${this.binaryPath} stats --json`, {
      encoding: "utf-8",
      timeout: 5000,
    });
    return JSON.parse(output);
  }

  /**
   * Check if the loop should continue based on OMNI's recommendation
   */
  shouldContinue(): boolean {
    try {
      const status = this.getStatus();
      return status.recommendation.action === "CONTINUE";
    } catch {
      // Fail open — if OMNI is unavailable, continue
      return true;
    }
  }

  /**
   * Get context pressure level
   */
  getContextPressure(): "Normal" | "Warning" | "Critical" {
    try {
      const status = this.getStatus();
      return status.context_pressure as "Normal" | "Warning" | "Critical";
    } catch {
      return "Normal";
    }
  }
}

/**
 * Standalone helper to check if loop should continue
 */
export function shouldContinueLoop(status: OmniHandoff): boolean {
  return (
    status.recommendation.action === "CONTINUE" ||
    status.recommendation.action === "COMPACT_OR_ESCALATE"
  );
}

/**
 * Example Mastra workflow integration
 */
export async function exampleMastraWorkflow(goal: string, maxIterations = 20) {
  const omni = new OmniClient();

  // Set environment for OMNI loop awareness
  process.env.OMNI_LOOP_ID = crypto.randomUUID();
  process.env.OMNI_LOOP_GOAL = goal;
  process.env.OMNI_LOOP_BUDGET = "100000";

  for (let i = 1; i <= maxIterations; i++) {
    process.env.OMNI_LOOP_ITERATION = String(i);

    if (!omni.shouldContinue()) {
      console.log(`Loop ended at iteration ${i}: ${omni.getStatus().recommendation.reason}`);
      break;
    }

    // ... your Mastra agent step here ...
    console.log(`Iteration ${i}: running agent...`);
  }

  // Print final stats
  const stats = omni.getStats();
  console.log(`Tokens saved: ${stats.tokens_saved}`);
}
