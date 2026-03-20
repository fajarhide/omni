import { beforeAll, describe, expect, test } from 'bun:test';
import { createOmniEngine } from '../test-helper.js';

const config = {
    rules: [],
    dsl_filters: [
        {
            name: 'pytest-summary-fail',
            trigger: ' failed, ',
            confidence: 0.95,
            rules: [
                { capture: '{failed} failed, {passed} passed in {duration}', action: 'keep' }
            ],
            output: 'pytest: {passed} passed | {failed} failed | {duration}'
        },
        {
            name: 'ruff-summary-pass',
            trigger: 'All checks passed!',
            confidence: 0.99,
            rules: [],
            output: 'ruff: all checks passed'
        },
        {
            name: 'ruff-summary-errors-plural',
            trigger: 'Found ',
            confidence: 0.96,
            rules: [
                { capture: 'Found {errors} errors.', action: 'keep' }
            ],
            output: 'ruff: {errors} errors'
        },
        {
            name: 'cargo-test-summary-pass',
            trigger: 'test result: ok.',
            confidence: 0.97,
            rules: [
                { capture: 'test result: ok. {passed} passed; {failed} failed; {ignored} ignored; {measured} measured; {filtered} filtered out; finished in {duration}', action: 'keep' }
            ],
            output: 'cargo test: {passed} passed | {failed} failed | {duration}'
        },
        {
            name: 'pnpm-install-summary',
            trigger: 'Progress: resolved',
            confidence: 0.93,
            rules: [
                { capture: 'Progress: resolved {resolved}, reused {reused}, downloaded {downloaded}, added {added}, done', action: 'keep' },
                { capture: 'Done in {duration}', action: 'keep' }
            ],
            output: 'pnpm: resolved {resolved} | reused {reused} | downloaded {downloaded} | added {added} | {duration}'
        },
        {
            name: 'zig-test-summary-pass',
            trigger: 'tests passed.',
            confidence: 0.92,
            rules: [
                { capture: 'All {passed} tests passed.', action: 'keep' }
            ],
            output: 'zig test: {passed} passed'
        },
        {
            name: 'go-test-summary-pass',
            trigger: 'ok\t',
            confidence: 0.9,
            rules: [
                { capture: 'ok\t{pkg}\t{duration}', action: 'keep' },
                { capture: 'ok\t{counted_pkg}\t{counted_duration}', action: 'count', as: 'passed_packages' }
            ],
            output: 'go test: {passed_packages} packages passed | last {pkg} | {duration}'
        }
    ]
};

let engine: { distill: (text: string) => string };

beforeAll(async () => {
    engine = await createOmniEngine(config);
});

describe('Polyglot templates', () => {
    test('distills pytest failure summary', () => {
        const output = engine.distill('1 failed, 9 passed in 0.12s');
        expect(output).toBe('pytest: 9 passed | 1 failed | 0.12s');
    });

    test('distills ruff pass summary', () => {
        const output = engine.distill('All checks passed!');
        expect(output).toBe('ruff: all checks passed');
    });

    test('distills ruff error summary', () => {
        const output = engine.distill('Found 3 errors.');
        expect(output).toBe('ruff: 3 errors');
    });

    test('distills cargo test summary', () => {
        const output = engine.distill('test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s');
        expect(output).toBe('cargo test: 12 passed | 0 failed | 0.03s');
    });

    test('distills pnpm install summary', () => {
        const input = [
            'Progress: resolved 132, reused 120, downloaded 4, added 12, done',
            'Done in 5.4s'
        ].join('\n');
        const output = engine.distill(input);
        expect(output).toBe('pnpm: resolved 132 | reused 120 | downloaded 4 | added 12 | 5.4s');
    });

    test('distills zig test pass summary', () => {
        const output = engine.distill('All 17 tests passed.');
        expect(output).toBe('zig test: 17 passed');
    });

    test('distills go test package summary', () => {
        const input = [
            'ok\tgithub.com/acme/project/internal/foo\t0.021s',
            'ok\tgithub.com/acme/project/internal/bar\t0.042s'
        ].join('\n');
        const output = engine.distill(input);
        expect(output).toBe('go test: 2 packages passed | last github.com/acme/project/internal/bar | 0.042s');
    });
});
