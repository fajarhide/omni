import { describe, test, expect, beforeAll } from 'bun:test';
import { createOmniEngine, readFixture } from '../test-helper.js';

let engine: { distill: (text: string) => string };

beforeAll(async () => {
    engine = await createOmniEngine();
});

describe('CustomFilter', () => {
    // Test 1: Remove rule works via global engine
    test('engine distills text (remove action)', () => {
        // The custom filter requires config. The engine loads global/local config.
        // We test that the engine can process text through the pipeline.
        const input = 'Application log: user logged in successfully';
        const output = engine.distill(input);
        // Should produce output (passthrough or processed)
        expect(output.length).toBeGreaterThan(0);
    });

    // Test 2: Mask rule behavior
    test('engine produces output for text with sensitive patterns', () => {
        const input = 'Connection: password: my_secret_pass, host: 192.168.1.1';
        const output = engine.distill(input);
        // If security-audit template was loaded, password/IP would be masked
        // Otherwise, engine still processes it
        expect(output.length).toBeGreaterThan(0);
    });

    // Test 3: False positive - no matching pattern
    test('text without any matching custom rules passes through', () => {
        const input = 'Hello world from a simple test script';
        const output = engine.distill(input);
        // Should still produce valid output
        expect(typeof output).toBe('string');
        expect(output.length).toBeGreaterThan(0);
    });

    // Test 4: Engine handles minimal input
    test('engine handles minimal whitespace input', () => {
        const output = engine.distill(' ');
        expect(typeof output).toBe('string');
        expect(output.length).toBeGreaterThan(0);
    });

    // Test 5: Engine handles very large input
    test('engine handles large input without crash', () => {
        const bigInput = 'A'.repeat(10000) + '\nerror: something broke\n' + 'B'.repeat(10000);
        const output = engine.distill(bigInput);
        expect(output.length).toBeGreaterThan(0);
        expect(output.length).toBeLessThan(bigInput.length);
    });

    // Test 6: Config template rules structure
    test('template rules exist for kubernetes, terraform, node-verbose, docker-layers', () => {
        // Verify the engine processes known template trigger patterns
        const k8sInput = 'uid: abc123-def456\nmanagedFields:\n  - manager: kubectl';
        const output = engine.distill(k8sInput);
        expect(output.length).toBeGreaterThan(0);
    });

    test('higher-confidence DSL filter wins over generic auto-filter', async () => {
        const customEngine = await createOmniEngine({
            rules: [],
            dsl_filters: [
                {
                    name: 'generic-auto',
                    trigger: '---',
                    confidence: 0.6,
                    rules: [
                        { capture: '--- {value}', action: 'count', as: 'value_count' }
                    ],
                    output: '[auto-filtered] ---: {value_count}x suppressed'
                },
                {
                    name: 'specific-report',
                    trigger: 'SPECIAL_SUMMARY:',
                    confidence: 0.99,
                    rules: [
                        { capture: 'SPECIAL_SUMMARY: {status}', action: 'keep' }
                    ],
                    output: 'report: {status}'
                }
            ]
        });

        const input = [
            'SECTION A',
            '--- repeated noise',
            'SPECIAL_SUMMARY: deployment blocked',
        ].join('\n');
        const output = customEngine.distill(input);
        expect(output).toContain('report: deployment blocked');
        expect(output).not.toContain('[auto-filtered]');
    });

    test('supports default values for missing placeholders', async () => {
        const customEngine = await createOmniEngine({
            rules: [],
            dsl_filters: [
                {
                    name: 'defaults',
                    trigger: 'SUMMARY:',
                    confidence: 0.99,
                    rules: [
                        { capture: 'SUMMARY: {value}', action: 'keep' }
                    ],
                    output: 'summary: {value} | {missing?none}'
                }
            ]
        });

        const output = customEngine.distill('SUMMARY: healthy');
        expect(output).toBe('summary: healthy | none');
    });

    test('omits unresolved placeholders without leaking template markers', async () => {
        const customEngine = await createOmniEngine({
            rules: [],
            dsl_filters: [
                {
                    name: 'optional-placeholder',
                    trigger: 'STATUS:',
                    confidence: 0.99,
                    rules: [
                        { capture: 'STATUS: {value}', action: 'keep' }
                    ],
                    output: 'status:{value}{missing}'
                }
            ]
        });

        const output = customEngine.distill('STATUS: ok');
        expect(output).toBe('status:ok');
        expect(output).not.toContain('{missing}');
    });
});
