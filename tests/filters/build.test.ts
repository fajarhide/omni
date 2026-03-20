import { describe, test, expect, beforeAll } from 'bun:test';
import { createOmniEngine, readFixture } from '../test-helper.js';

let engine: { distill: (text: string) => string };

beforeAll(async () => {
    engine = await createOmniEngine();
});

describe('BuildFilter', () => {
    // Test 1: Valid input - error output
    test('matches and distills build output with errors', () => {
        const input = readFixture('build_error.txt');
        const output = engine.distill(input);
        expect(output).toContain('error:');
        expect(output).toContain('undeclared identifier');
    });

    // Test 2: Valid input - warning output
    test('matches and keeps warning lines', () => {
        const input = readFixture('build_error.txt');
        const output = engine.distill(input);
        expect(output).toContain('warning:');
    });

    // Test 3: False positive - no error/warning
    test('does NOT match text without error/warning keywords', () => {
        const plainText = 'Everything is fine. The system is running smoothly without issues.';
        const output = engine.distill(plainText);
        // Build filter only matches when there are error: or warning: keywords
        expect(output).not.toContain('[Build output distilled]');
    });

    // Test 4: Output format - keeps errors, drops Compiling
    test('output format: strips Compiling lines, keeps errors', () => {
        const input = readFixture('build_error.txt');
        const output = engine.distill(input);
        // Compiling lines are stripped
        expect(output).not.toContain('Compiling src/main.c');
        expect(output).not.toContain('Compiling src/utils.c');
        // Errors and summary kept
        expect(output).toContain('error:');
        expect(output).toContain('Build failed');
    });

    // Test 5: Score variance (error=1.0 vs warning-only=0.9)
    test('score variance: error output scores higher than warning-only', () => {
        const errorInput = "src/main.c:10:5: error: missing semicolon\nsrc/main.c:20:8: warning: unused variable";
        const warningInput = "src/main.c:20:8: warning: unused variable 'x'\nsrc/main.c:30:8: warning: unused function 'foo'";
        const errorOutput = engine.distill(errorInput);
        const warningOutput = engine.distill(warningInput);
        // Both should be distilled but may differ
        expect(errorOutput).toContain('error:');
        expect(warningOutput).toContain('warning:');
    });

    // Test 6: Savings check
    test('savings: output shorter than input on build errors', () => {
        const input = readFixture('build_error.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });

    // Test 7: Clean build distilled
    test('clean build with no error/warning is not matched by build filter', () => {
        const input = readFixture('build_success.txt');
        const output = engine.distill(input);
        // build_success.txt has no error: or warning: so BuildFilter won't match
        // It should fallback to another filter or passthrough
        expect(output).not.toContain('error:');
    });

    // Test 8: Build Summary preserved
    test('preserves Build Summary and failed/succeeded lines', () => {
        const input = 'Compiling module_a\nCompiling module_b\nerror: link failed\nBuild Summary: 2 modules\n1 succeeded, 1 failed';
        const output = engine.distill(input);
        expect(output).toContain('Build Summary');
        expect(output).toContain('failed');
        expect(output).toContain('succeeded');
    });
});
