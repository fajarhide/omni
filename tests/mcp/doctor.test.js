import test from 'node:test';
import assert from 'node:assert';
import { spawnSync } from 'child_process';
import path from 'path';
import fs from 'fs';
import os from 'os';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const doctorBin = path.join(__dirname, '../../core/zig-out/bin/omni');

test('CLI Doctor - warns on overly generic DSL triggers', () => {
    const tmpHome = fs.mkdtempSync(path.join(os.tmpdir(), 'omni-doctor-home-'));
    const tmpProject = fs.mkdtempSync(path.join(os.tmpdir(), 'omni-doctor-project-'));

    fs.writeFileSync(
        path.join(tmpProject, 'omni_config.json'),
        JSON.stringify({
            rules: [],
            dsl_filters: [
                {
                    name: 'too-generic',
                    trigger: 'failed',
                    confidence: 0.9,
                    rules: [
                        { capture: 'failed {value}', action: 'keep' }
                    ],
                    output: 'failure: {value}'
                }
            ]
        }, null, 2)
    );

    try {
        const result = spawnSync(doctorBin, ['doctor'], {
            cwd: tmpProject,
            env: { ...process.env, HOME: tmpHome },
            encoding: 'utf8'
        });

        assert.strictEqual(result.status, 0, `doctor should exit cleanly: ${result.stderr}`);
        assert.match(result.stdout, /Filter Diagnostics:/);
        assert.match(result.stdout, /too-generic/);
        assert.match(result.stdout, /too generic; prefer a more specific multi-token trigger/);
    } finally {
        fs.rmSync(tmpProject, { recursive: true, force: true });
        fs.rmSync(tmpHome, { recursive: true, force: true });
    }
});

test('CLI Doctor - strict mode fails on overly generic DSL triggers', () => {
    const tmpHome = fs.mkdtempSync(path.join(os.tmpdir(), 'omni-doctor-home-'));
    const tmpProject = fs.mkdtempSync(path.join(os.tmpdir(), 'omni-doctor-project-'));

    fs.writeFileSync(
        path.join(tmpProject, 'omni_config.json'),
        JSON.stringify({
            rules: [],
            dsl_filters: [
                {
                    name: 'too-generic',
                    trigger: 'failed',
                    confidence: 0.9,
                    rules: [
                        { capture: 'failed {value}', action: 'keep' }
                    ],
                    output: 'failure: {value}'
                }
            ]
        }, null, 2)
    );

    try {
        const result = spawnSync(doctorBin, ['doctor', '--strict'], {
            cwd: tmpProject,
            env: { ...process.env, HOME: tmpHome },
            encoding: 'utf8'
        });

        assert.strictEqual(result.status, 1, 'doctor --strict should fail when warnings exist');
        assert.match(result.stdout, /Strict mode failed/);
    } finally {
        fs.rmSync(tmpProject, { recursive: true, force: true });
        fs.rmSync(tmpHome, { recursive: true, force: true });
    }
});
