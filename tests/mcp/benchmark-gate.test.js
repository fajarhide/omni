import test from 'node:test';
import assert from 'node:assert';
import { spawnSync } from 'child_process';
import path from 'path';
import fs from 'fs';
import os from 'os';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const benchmarkScript = path.join(__dirname, '../folded/benchmark-fixtures.mjs');

test('Benchmark Gate - strict mode passes against current baseline', () => {
    const result = spawnSync('node', [benchmarkScript, '--strict'], {
        cwd: path.join(__dirname, '../..'),
        encoding: 'utf8'
    });

    assert.strictEqual(result.status, 0, `strict benchmark should pass: ${result.stdout}\n${result.stderr}`);
});

test('Benchmark Gate - strict mode fails on regressed baseline expectations', () => {
    const tmpBaselineDir = fs.mkdtempSync(path.join(os.tmpdir(), 'omni-benchmark-'));
    const baselinePath = path.join(tmpBaselineDir, 'baseline.json');

    fs.writeFileSync(baselinePath, JSON.stringify({
        version: 1,
        average_saved_fraction: 0.99,
        cases: [
            { name: 'npm-install', saved_fraction: 0.95, quality: 'good' },
            { name: 'docker-build', saved_fraction: 0.99, quality: 'good' }
        ]
    }, null, 2));

    try {
        const result = spawnSync('node', [benchmarkScript, '--strict', `--baseline=${baselinePath}`], {
            cwd: path.join(__dirname, '../..'),
            encoding: 'utf8'
        });

        assert.strictEqual(result.status, 1, 'strict benchmark should fail on impossible baseline');
        assert.match(result.stdout, /Regression failures:/);
    } finally {
        fs.rmSync(tmpBaselineDir, { recursive: true, force: true });
    }
});
