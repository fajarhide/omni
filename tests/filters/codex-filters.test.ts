import { describe, test, expect, beforeAll } from 'bun:test';
import { createOmniEngine, readFixture } from '../test-helper.js';

let engine: { distill: (text: string) => string };

beforeAll(async () => {
    engine = await createOmniEngine();
});

describe('TokenSavingsValidation', () => {
    test('npm install full output is compressed', () => {
        const input = readFixture('npm_install_full.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });

    test('docker build output is compressed', () => {
        const input = readFixture('docker_build.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });

    test('jest pass output is compressed', () => {
        const input = readFixture('jest_pass.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });

    test('jest fail output is compressed', () => {
        const input = readFixture('jest_fail.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });

    test('vite build output is compressed', () => {
        const input = readFixture('vite_build.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });

    test('webpack build output is compressed', () => {
        const input = readFixture('webpack_build.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });

    test('tsc errors output is compressed', () => {
        const input = readFixture('tsc_errors.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });

    test('eslint errors output is compressed', () => {
        const input = readFixture('eslint_errors.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });

    test('pytest fail output is compressed', () => {
        const input = readFixture('pytest_fail.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });
});

describe('ExistingBuiltinFilters', () => {
    test('git diff is distilled', () => {
        const input = readFixture('git_diff.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });

    test('git dirty status is distilled', () => {
        const input = readFixture('git_dirty.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });

    test('git clean status is distilled', () => {
        const input = readFixture('git_clean.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });

    test('docker build with layers is distilled', () => {
        const input = readFixture('docker_build.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });

    test('docker compose is distilled', () => {
        const input = readFixture('docker_compose.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });

    test('npm install is distilled', () => {
        const input = readFixture('npm_install.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });

    test('yarn install is distilled', () => {
        const input = readFixture('yarn_install.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });

    test('build errors are distilled', () => {
        const input = readFixture('build_error.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });

    test('build success is distilled', () => {
        const input = readFixture('build_success.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });
});

describe('OutputFormatValidation', () => {
    test('jest pass output contains passed count', () => {
        const input = readFixture('jest_pass.txt');
        const output = engine.distill(input);
        expect(output).toContain('passed');
    });

    test('jest fail output contains failed', () => {
        const input = readFixture('jest_fail.txt');
        const output = engine.distill(input);
        expect(output).toContain('failed');
    });

    test('tsc errors output contains diagnostics info', () => {
        const input = readFixture('tsc_errors.txt');
        const output = engine.distill(input);
        expect(output).toContain('TS2769');
    });

    test('eslint output contains problems count', () => {
        const input = readFixture('eslint_errors.txt');
        const output = engine.distill(input);
        expect(output).toContain('problems');
    });
});

describe('MultiToolOutputCompression', () => {
    test('large multi-line outputs are compressed', () => {
        const testCases = [
            { name: 'npm_install_full', fixture: 'npm_install_full.txt' },
            { name: 'tsc_errors', fixture: 'tsc_errors.txt' },
            { name: 'eslint_errors', fixture: 'eslint_errors.txt' },
            { name: 'jest_pass', fixture: 'jest_pass.txt' },
            { name: 'jest_fail', fixture: 'jest_fail.txt' },
            { name: 'vite_build', fixture: 'vite_build.txt' },
            { name: 'webpack_build', fixture: 'webpack_build.txt' },
            { name: 'pytest_fail', fixture: 'pytest_fail.txt' },
        ];

        for (const tc of testCases) {
            const input = readFixture(tc.fixture);
            const output = engine.distill(input);
            expect(output.length).toBeLessThan(input.length),
                `${tc.name}: output (${output.length}) should be less than input (${input.length})`;
        }
    });
});

describe('EdgeCaseHandling', () => {
    test('tsc errors fixture is distilled with full content', () => {
        const input = readFixture('tsc_errors.txt');
        const output = engine.distill(input);
        expect(output).toContain('TS2769');
        expect(output.length).toBeLessThan(input.length);
    });

    test('multi-line npm install is distilled', () => {
        const input = readFixture('npm_install_full.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });
});
