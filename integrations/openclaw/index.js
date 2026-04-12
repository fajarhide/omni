const { exec } = require('child_process');
const util = require('util');
const execPromise = util.promisify(exec);

/**
 * OMNI Plugin for OpenClaw
 * 
 * Provides a highly-efficient shell tool that uses OMNI's
 * distillation engine to reduce token usage.
 */
module.exports = function(sdk) {
  if (!sdk) return;

  const config = (typeof sdk.getConfig === 'function') ? sdk.getConfig() : {};
  const omniPath = config.omniPath || 'omni';

  sdk.registerTool({
    id: "omni_shell",
    description: "Run a shell command with intelligent OMNI filtering. Use this for noisy commands (git, npm, cargo, docker) to save 80-90% of token costs.",
    schema: {
      type: "object",
      properties: {
        command: {
          type: "string",
          description: "The shell command to execute"
        }
      },
      required: ["command"]
    },
    handler: async ({ command }) => {
      try {
        // We wrap the command in `omni exec`
        // The -- ensures that omni doesn't parse command flags as its own
        const fullCommand = `${omniPath} exec -- ${command}`;
        
        const { stdout, stderr } = await execPromise(fullCommand);
        
        let result = stdout || "";
        if (stderr && stderr.trim()) {
          result += `\n[stderr]\n${stderr}`;
        }

        return {
          content: result || "(No output from OMNI)",
          role: "tool"
        };
      } catch (error) {
        sdk.log(`OMNI Error: ${error.message}`, "error");
        return {
          content: `Error running OMNI: ${error.message}\n${error.stderr || ""}`,
          isError: true
        };
      }
    }
  });

  // Optional: Provide a direct rewind tool
  sdk.registerTool({
    id: "omni_rewind",
    description: "Retrieve full archived output from OMNI if the distilled summary was insufficient.",
    schema: {
      type: "object",
      properties: {
        hash: {
          type: "string",
          description: "The 8-character hash provided in the OMNI summary"
        }
      },
      required: ["hash"]
    },
    handler: async ({ hash }) => {
      try {
        const { stdout } = await execPromise(`${omniPath} rewind ${hash}`);
        return {
          content: stdout || "No archive found for this hash.",
          role: "tool"
        };
      } catch (error) {
        return {
          content: `Failed to retrieve OMNI archive: ${error.message}`,
          isError: true
        };
      }
    }
  });

  sdk.log("OMNI Signal Engine plugin loaded successfully.");
};
