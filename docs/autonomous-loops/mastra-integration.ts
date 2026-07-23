/**
 * OMNI Mastra/Vercel AI SDK Integration
 *
 * TypeScript helper for consuming OMNI from a Mastra workflow.
 *
 * The handoff-driven loop control this file used to expose is gone (#180).
 * It shelled out to `omni handoff --json`, which #164 removed as a CLI
 * subcommand. `omni_handoff` still exists as an **MCP tool** — a workflow that
 * already speaks MCP should call it there. There is no shell equivalent, so
 * `getStatus`, `shouldContinue`, `getContextPressure` and `shouldContinueLoop`
 * were deleted rather than stubbed: each wrapped the call in a `catch` that
 * returned `true` / `"Normal"`, so once the command was gone they would have
 * reported a healthy loop forever from state they never read.
 *
 * Usage:
 *   import { OmniClient } from './mastra-integration';
 *
 *   const omni = new OmniClient();
 *   // ... run agent steps ...
 *   console.log(omni.getStats().tokens_saved);
 */

import { execSync } from "child_process";

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
   * Get token savings statistics
   */
  getStats(): OmniStats {
    const output = execSync(`${this.binaryPath} stats --json`, {
      encoding: "utf-8",
      timeout: 5000,
    });
    return JSON.parse(output);
  }

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

  // Runs to maxIterations: the per-iteration DONE / ESCALATE checkpoint was
  // handoff-driven and is MCP-only now (#180). Add your own exit condition, or
  // call the `omni_handoff` MCP tool if this workflow has an MCP client.
  for (let i = 1; i <= maxIterations; i++) {
    process.env.OMNI_LOOP_ITERATION = String(i);

    // ... your Mastra agent step here ...
    console.log(`Iteration ${i}: running agent...`);
  }

  // Print final stats
  const stats = omni.getStats();
  console.log(`Tokens saved: ${stats.tokens_saved}`);
}
