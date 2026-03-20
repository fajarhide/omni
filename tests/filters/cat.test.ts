import { describe, test, expect, beforeAll } from 'bun:test';
import { createOmniEngine, readFixture } from '../test-helper.js';

let engine: { distill: (text: string) => string };

beforeAll(async () => {
    engine = await createOmniEngine();
});

describe('CatFilter', () => {
    // Test 1: Valid input - Markdown document
    test('matches and distills markdown document (headers + lists)', () => {
        const input = readFixture('cat_readme.txt');
        const output = engine.distill(input);
        expect(output).toContain('# OMNI Project');
        expect(output).toContain('## Features');
        expect(output).toContain('### Getting Started');
        expect(output).toContain('- Fast distillation');
    });

    // Test 2: Valid input - Raw content summary
    test('summarizes raw content without headers', () => {
        const input = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7\nLine 8\nLine 9\nLine 10\n" +
                      "Line 11\nLine 12\nLine 13\nLine 14\nLine 15\nLine 16\nLine 17\nLine 18\nLine 19\nLine 20";
        const output = engine.distill(input);
        expect(output).toMatch(/cat distilled|distilled/i);
    });

    // Test 3: Cat always matches (but low score for noise)
    test('cat filter matches everything but short noise gets low score', () => {
        const shortNoise = 'ok';
        const output = engine.distill(shortNoise);
        // Short input gets low cat score (0.1) — may be dropped or manifest
        expect(output.length).toBeGreaterThan(0);
    });

    // Test 4: Output format - headers extracted
    test('output format: extracts markdown headers from document', () => {
        const input = readFixture('cat_readme.txt');
        const output = engine.distill(input);
        // Should contain headers but not regular paragraph text
        expect(output).toContain('#');
        expect(output).toContain('Features');
    });

    // Test 5: Score variance (doc with headers vs plain text)
    test('score variance: markdown doc vs plain text produce different outputs', () => {
        const mdOutput = engine.distill(readFixture('cat_readme.txt'));
        const plainOutput = engine.distill(readFixture('cat_plain.txt'));
        expect(mdOutput).not.toBe(plainOutput);
    });

    // Test 6: Savings check
    test('savings: markdown output shorter than input', () => {
        const input = readFixture('cat_readme.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThanOrEqual(input.length);
    });

    // Test 7: Large file handling
    test('handles large plain text files', () => {
        const input = readFixture('cat_plain.txt');
        const output = engine.distill(input);
        expect(output.length).toBeGreaterThan(0);
        // Large headerless content should produce a summary
        expect(output).toMatch(/cat distilled|distilled|lines/i);
    });

    test('large plain text is distilled directly without a manifest wrapper', () => {
        const input = readFixture('cat_plain.txt');
        const output = engine.distill(input);
        expect(output).not.toContain('[OMNI Context Manifest');
        expect(output).toContain('cat distilled');
    });

    // Test 8: List item extraction
    test('extracts list items from markdown', () => {
        const input = '# My Doc\n- Item one\n- Item two\n* Star item\nSome paragraph text here that is not a list.';
        const output = engine.distill(input);
        expect(output).toContain('# My Doc');
        expect(output).toContain('- Item one');
    });
});
