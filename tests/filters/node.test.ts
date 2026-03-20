import { describe, test, expect, beforeAll } from 'bun:test';
import { createOmniEngine, readFixture } from '../test-helper.js';

let engine: { distill: (text: string) => string };

beforeAll(async () => {
    engine = await createOmniEngine();
});

describe('NodeFilter', () => {
    // Test 1: Valid input - npm install
    test('matches and distills npm install output', () => {
        const input = readFixture('npm_install.txt');
        const output = engine.distill(input);
        expect(output).toContain('npm:');
        expect(output).toContain('154 packages');
    });

    // Test 2: Valid input - yarn install
    test('matches and distills yarn install output', () => {
        const input = readFixture('yarn_install.txt');
        const output = engine.distill(input);
        expect(output).toContain('Done in');
    });

    // Test 3: False positive - plain text
    test('does NOT match text without npm/yarn keywords', () => {
        const plainText = 'The quick brown fox jumps over the lazy dog. No software here.';
        const output = engine.distill(plainText);
        // Should not be handled by node filter
        expect(output).not.toContain('[Node/NPM noise distilled]');
    });

    // Test 4: Output format - summary lines kept
    test('output format: keeps summary lines only', () => {
        const input = readFixture('npm_install.txt');
        const output = engine.distill(input);
        expect(output).toContain('npm:');
        expect(output).toContain('154 packages');
        // npm notice lines should be stripped (they're noise)
        expect(output).not.toContain('npm notice New major');
        expect(output).not.toContain('Changelog');
    });

    // Test 5: Score variance
    test('score variance: npm install vs yarn output', () => {
        const npmOutput = engine.distill(readFixture('npm_install.txt'));
        const yarnOutput = engine.distill(readFixture('yarn_install.txt'));
        // Both handled by node filter but different content
        expect(npmOutput).not.toBe(yarnOutput);
    });

    // Test 6: Savings check
    test('output is shorter than input (savings)', () => {
        const input = readFixture('npm_install.txt');
        const output = engine.distill(input);
        expect(output.length).toBeLessThan(input.length);
    });

    // Test 7: Vulnerability report
    test('emits a short npm summary for vulnerability-heavy output', () => {
        const input = 'added 200 packages in 3s\n12 packages are looking for funding\nfound 3 vulnerabilities (1 moderate, 2 high)';
        const output = engine.distill(input);
        expect(output).toContain('npm: 200 packages added | 3s | 3 vulnerabilities');
        expect(output.length).toBeLessThan(input.length);
    });

    // Test 8: Error handling
    test('emits a compressed npm summary for install errors', () => {
        const input = 'added 10 packages in 1s\nerror ERESOLVE unable to resolve dependency tree\nERR! peer dep missing: react@^18';
        const output = engine.distill(input);
        expect(output).toContain('npm: 10 packages added | 1s');
        expect(output.length).toBeLessThan(input.length);
    });
});
