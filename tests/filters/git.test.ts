import { describe, test, expect, beforeAll } from 'bun:test';
import { createOmniEngine, readFixture } from '../test-helper.js';

let engine: { distill: (text: string) => string };

beforeAll(async () => {
    engine = await createOmniEngine();
});

describe('GitFilter', () => {
    // Test 1: Valid input - Clean Status
    test('matches and distills clean git status', () => {
        const input = readFixture('git_clean.txt');
        const output = engine.distill(input);
        expect(output).toContain('git');
        expect(output).toContain('clean');
        expect(output.length).toBeLessThan(input.length);
    });

    // Test 2: Valid input - Dirty Status
    test('matches and distills dirty git status', () => {
        const input = readFixture('git_dirty.txt');
        const output = engine.distill(input);
        expect(output).toContain('git');
        expect(output.length).toBeGreaterThan(0);
        expect(output.length).toBeLessThan(input.length);
    });

    // Test 3: Valid input - Diff output
    test('matches and distills git diff', () => {
        const input = readFixture('git_diff.txt');
        const output = engine.distill(input);
        expect(output).toContain('git diff:');
        expect(output).toContain('core/src/main.zig');
        // Noise lines removed
        expect(output).not.toContain('index 1ea518f');
        expect(output).not.toContain('--- a/');
    });

    // Test 4: Valid input - Log output
    test('matches and distills git log (strips hashes)', () => {
        const input = readFixture('git_log.txt');
        const output = engine.distill(input);
        expect(output).toContain('fix git log distillation');
        expect(output).toContain('docs: update changelog');
        // Commit hashes should be stripped
        expect(output).not.toContain('a1b2c3d');
        expect(output).not.toContain('b2c3d4e');
    });

    // Test 5: Valid input - Add verbose
    test('matches and distills git add verbose output', () => {
        const input = readFixture('git_add.txt');
        const output = engine.distill(input);
        expect(output).toContain('git');
        expect(output).toMatch(/added.*files|6 files/i);
    });

    // Test 6: False positive check
    test('does NOT match plain prose text', () => {
        const plainText = 'The weather is sunny today. Let us go for a walk in the park.';
        const output = engine.distill(plainText);
        // Should NOT be processed by git filter — output should be passthrough or cat
        expect(output).not.toMatch(/^git:/);
    });

    // Test 7: Output format - status summary
    test('output format: dirty status still emits a compact signal', () => {
        const input = readFixture('git_dirty.txt');
        const output = engine.distill(input);
        expect(output.length).toBeGreaterThan(0);
        expect(output).not.toContain('[OMNI Context Manifest');
        expect(output).not.toContain('npm:');
    });

    // Test 8: Output format - diff is normalized into a compact summary
    test('output format: diff emits normalized change summary', () => {
        const input = readFixture('git_diff.txt');
        const output = engine.distill(input);
        expect(output).toContain('1 hunks');
    });

    // Test 9: Score variance
    test('score variance: different inputs produce different outputs', () => {
        const cleanOutput = engine.distill(readFixture('git_clean.txt'));
        const dirtyOutput = engine.distill(readFixture('git_dirty.txt'));
        const diffOutput = engine.distill(readFixture('git_diff.txt'));
        // All three should produce meaningfully different outputs
        expect(cleanOutput).not.toBe(dirtyOutput);
        expect(dirtyOutput).not.toBe(diffOutput);
        expect(cleanOutput).not.toBe(diffOutput);
    });

    // Test 10: Savings >= 60%
    test('savings >= 60% on dirty status', () => {
        const input = readFixture('git_dirty.txt');
        const output = engine.distill(input);
        const savings = 1 - (output.length / input.length);
        expect(savings).toBeGreaterThanOrEqual(0.6);
    });
});
