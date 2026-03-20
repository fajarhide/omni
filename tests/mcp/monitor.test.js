import test from 'node:test';
import assert from 'node:assert';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '../..');
const omniBin = path.join(repoRoot, 'core', 'zig-out', 'bin', 'omni');

test('CLI Monitor - native CLI agent is distinct from codex profile family', () => {
    const tempHome = fs.mkdtempSync(path.join(os.tmpdir(), 'omni-monitor-home-'));
    const tempProject = fs.mkdtempSync(path.join(os.tmpdir(), 'omni-monitor-project-'));

    try {
        fs.writeFileSync(
            path.join(tempProject, 'omni_config.json'),
            JSON.stringify({
                rules: [],
                dsl_filters: [
                    {
                        name: 'codex-npm-install-summary',
                        trigger: 'added ',
                        confidence: 0.95,
                        rules: [
                            { capture: 'added {packages} packages', action: 'keep' },
                            { capture: 'in {duration}', action: 'keep' },
                        ],
                        output: 'npm: {packages} packages added | {duration?unknown}',
                    },
                ],
            }, null, 2),
        );

        const distill = spawnSync(omniBin, {
            cwd: tempProject,
            env: { ...process.env, HOME: tempHome },
            input: 'added 42 packages in 3.2s\n',
            encoding: 'utf8',
        });

        assert.strictEqual(distill.status, 0, `distill should pass: ${distill.stderr}`);

        const monitor = spawnSync(omniBin, ['monitor', '--prune-noise'], {
            cwd: tempProject,
            env: { ...process.env, HOME: tempHome },
            encoding: 'utf8',
        });

        assert.strictEqual(monitor.status, 0, `monitor should pass: ${monitor.stderr}`);
        assert.match(monitor.stdout, /PROFILE BREAKDOWN/);
        assert.match(monitor.stdout, /codex/);
        assert.match(monitor.stdout, /native-cli/);
        assert.doesNotMatch(monitor.stdout, /\bCLI\b/);
        assert.doesNotMatch(monitor.stdout, /\bcustom\b/);
    } finally {
        fs.rmSync(tempHome, { recursive: true, force: true });
        fs.rmSync(tempProject, { recursive: true, force: true });
    }
});
