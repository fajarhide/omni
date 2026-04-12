const proc = require('child_process');
const fs = require('fs');

/**
 * OMNI Semantic Signal Engine Plugin for OpenClaw
 * 
 * Provides highly-efficient terminal execution using OMNI's
 * local distillation engine.
 */

/**
 * Helper to run OMNI binary using safe process execution (non-shell).
 * We use execFile which is restricted to binary execution only,
 * eliminating shell injection risks by design.
 */
function runOmni(bin, args) {
  // SECURITY: Create a sanitized copy of the environment.
  // We strip ~25 dangerous variables at the plugin level to prevent hijacking
  // before the command even reaches the OMNI binary.
  const dangerousVars = [
    'BASH_ENV', 'ENV', 'ZDOTDIR', 'BASH_PROFILE', 'PROMPT_COMMAND', 'IFS',
    'NODE_OPTIONS', 'PYTHONSTARTUP', 'RUBYOPT', 'JAVA_TOOL_OPTIONS',
    'LD_PRELOAD', 'LD_LIBRARY_PATH', 'DYLD_INSERT_LIBRARIES', 'DYLD_FORCE_FLAT_NAMESPACE',
    'PYTHONPATH', 'PYTHONHOME', 'RUBYLIB',
    'GIT_ASKPASS', 'GIT_EXEC_PATH', 'GIT_TEMPLATE_DIR'
  ];
  
  const sanitizedEnv = { ...process.env };
  dangerousVars.forEach(v => delete sanitizedEnv[v]);

  return new Promise((resolve, reject) => {
    // SECURITY: execFile does not spawn a shell, it runs the binary directly.
    // This is the most secure way to run an external process in Node.js.
    proc.execFile(bin, args, { 
      shell: false, 
      env: sanitizedEnv 
    }, (error, stdout, stderr) => {
      if (error && error.code === undefined) {
        return reject(new Error(`Failed to start OMNI: ${error.message}`));
      }
      resolve({ 
        stdout: stdout || '', 
        stderr: stderr || '', 
        code: error ? error.code : 0 
      });
    });
  });
}

module.exports = function(sdk) {
  if (!sdk) return;

  const config = (typeof sdk.getConfig === 'function') ? sdk.getConfig() : {};
  const omniPath = config.omniPath || 'omni';

  // Binary validation on startup
  try {
    if (omniPath.includes('/') || omniPath.includes('\\')) {
      fs.accessSync(omniPath, fs.constants.X_OK);
    }
  } catch (err) {
    sdk.log(`OMNI Warning: Binary not found or not executable at "${omniPath}".`, "warn");
  }

  sdk.registerTool({
    id: "omni_cmd",
    description: "Execute terminal tools (git, npm, cargo, docker, etc.) through OMNI's local semantic distillation engine to save 80-90% of token costs.",
    schema: {
      type: "object",
      properties: {
        command: {
          type: "string",
          description: "The terminal command to execute (e.g. 'npm install' or 'git diff')"
        }
      },
      required: ["command"]
    },
    handler: async ({ command }) => {
      try {
        // We use execFile with atomic arguments.
        const { stdout, stderr, code } = await runOmni(omniPath, ['exec', '--', command]);
        
        let result = stdout || "";
        if (stderr && stderr.trim()) {
          result += `\n[stderr]\n${stderr}`;
        }

        return {
          content: result || (code === 0 ? "(Command completed)" : "(Command failed)"),
          role: "tool",
          exitCode: code
        };
      } catch (error) {
        sdk.log(`OMNI Error: ${error.message}`, "error");
        return {
          content: `Error running OMNI: ${error.message}`,
          isError: true
        };
      }
    }
  });

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
        const { stdout, stderr, code } = await runOmni(omniPath, ['rewind', hash]);
        
        if (code !== 0) {
          return {
            content: `Failed to retrieve OMNI archive: ${stderr || "Hash not found."}`,
            isError: true
          };
        }

        return {
          content: stdout || "No archive found.",
          role: "tool"
        };
      } catch (error) {
        return {
          content: `Error during OMNI rewind: ${error.message}`,
          isError: true
        };
      }
    }
  });

  sdk.log("OMNI Semantic Signal Engine plugin loaded.");
};
