import test from 'node:test';
import assert from 'node:assert';
import { spawnSync } from 'child_process';
import path from 'path';
import fs from 'fs';
import os from 'os';
import crypto from 'crypto';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const serverPath = path.join(__dirname, '../../dist/index.js');
test('MCP Server - Hook Integrity Check', async (t) => {
    const tmpHome = fs.mkdtempSync(path.join(os.tmpdir(), 'omni-test-'));
    const omniDir = path.join(tmpHome, '.omni');
    const hooksDir = path.join(omniDir, 'hooks');
    const shaFile = path.join(omniDir, 'hooks.sha256');
    
    fs.mkdirSync(hooksDir, { recursive: true });

    try {
        await t.test('Passes with no hooks', () => {
            const result = spawnSync('node', [serverPath], {
                env: { ...process.env, HOME: tmpHome }
            });
            assert.strictEqual(result.status, 0, 'Should pass when no hooks exist');
        });

        await t.test('Detects mismatch', () => {
            const hookFile = path.join(hooksDir, 'test.sh');
            fs.writeFileSync(hookFile, 'echo "secure"');
            
            const hashes = { "test.sh": "wrong-hash" };
            fs.writeFileSync(shaFile, JSON.stringify(hashes));

            const result = spawnSync('node', [serverPath], {
                env: { ...process.env, HOME: tmpHome }
            });
            assert.strictEqual(result.status, 1, 'Should fail on hash mismatch');
            assert.ok(result.stderr.toString().includes('Security Alert: Hook integrity mismatch'), 'Should log security alert');
        });

        await t.test('Detects untrusted file', () => {
            const hookFile = path.join(hooksDir, 'test.sh');
            const content = 'echo "secure"';
            fs.writeFileSync(hookFile, content);
            const hash = crypto.createHash('sha256').update(content).digest('hex');
            
            const hashes = { "test.sh": hash };
            fs.writeFileSync(shaFile, JSON.stringify(hashes));

            // Add an untrusted file
            fs.writeFileSync(path.join(hooksDir, 'untrusted.sh'), 'echo "evil"');

            const result = spawnSync('node', [serverPath], {
                env: { ...process.env, HOME: tmpHome }
            });
            assert.strictEqual(result.status, 1, 'Should fail on untrusted file');
            assert.ok(result.stderr.toString().includes('Security Alert: New untrusted hook file detected'), 'Should log untrusted file alert');
        });

    } finally {
        fs.rmSync(tmpHome, { recursive: true, force: true });
    }
});
