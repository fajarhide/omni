import test from 'node:test';
import assert from 'node:assert';
import { spawn } from 'child_process';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const serverPath = path.join(__dirname, '../../dist/index.js');

test('MCP Server - Startup', (t, done) => {
    // We just verify that the server starts and emits no immediate errors
    // Since it's an MCP server, it expects stdio interaction.
    const server = spawn('node', [serverPath], {
        env: { ...process.env, OMNI_SKIP_INIT: '1' } // Useful if we had such flag
    });

    let errorOutput = '';
    server.stderr.on('data', (data) => {
        errorOutput += data.toString();
    });

    // Give it a second to fail if it's going to
    setTimeout(() => {
        const cleanError = errorOutput
            .split('\n')
            .filter(line => !line.includes('ExperimentalWarning: WASI') && !line.includes('--trace-warnings'))
            .join('\n')
            .trim();
        assert.strictEqual(cleanError, '', `Server should not have stderr on startup: ${errorOutput}`);
        server.kill();
        done();
    }, 1000);
});
