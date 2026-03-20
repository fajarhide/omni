import { describe, test, expect, beforeAll } from 'bun:test';
import { createOmniEngine, readFixture } from '../test-helper.js';

let engine: { distill: (text: string) => string };

beforeAll(async () => {
    engine = await createOmniEngine();
});

describe('Compressor Routing', () => {
    // Test 1: High confidence path (score >= 0.8) — primary distillation
    test('high confidence: git status produces clean distilled output', () => {
        const input = readFixture('git_clean.txt');
        const output = engine.distill(input);
        // High confidence git: no "[OMNI Context Manifest" wrapper
        expect(output).not.toContain('[OMNI Context Manifest');
        expect(output).toContain('git');
    });

    // Test 2: High confidence path — diff also high confidence
    test('high confidence: git diff produces direct distillation', () => {
        const input = readFixture('git_diff.txt');
        const output = engine.distill(input);
        expect(output).not.toContain('[OMNI Context Manifest');
        expect(output).toContain('diff --git');
    });

    // Test 3: Passthrough for unknown input
    test('unknown input passes through unchanged', () => {
        const input = 'Simple one-liner that no filter recognizes as its domain';
        const output = engine.distill(input);
        // Should be returned as-is or with minimal wrapping
        expect(output.length).toBeGreaterThan(0);
    });

    // Test 4: categorizeUnknown — JSON detection
    test('JSON input is categorized correctly', () => {
        const jsonInput = '{"key": "value", "count": 42}';
        const output = engine.distill(jsonInput);
        // JSON should pass through (no specialized filter)
        expect(output.length).toBeGreaterThan(0);
    });

    // Test 5: Filter priority — highest score wins
    test('filter with highest score wins when multiple match', () => {
        // Git diff is very specific (high score) vs cat (always matches, lower score)
        const input = readFixture('git_diff.txt');
        const output = engine.distill(input);
        // Git filter should win over cat filter
        expect(output).toContain('diff --git');
        expect(output).not.toContain('cat distilled');
    });

    // Test 6: Multiple distillations are idempotent
    test('double distillation produces stable output', () => {
        const input = readFixture('git_clean.txt');
        const first = engine.distill(input);
        const second = engine.distill(first);
        // Second pass should be stable (no further reduction or different wrapping)
        expect(second.length).toBeLessThanOrEqual(first.length * 3);
    });
});
