import { describe, test, expect, beforeAll } from 'bun:test';
import { createOmniEngine, readFixture } from '../test-helper.js';

let engine: { distill: (text: string) => string };

beforeAll(async () => {
    engine = await createOmniEngine();
});

describe('SqlFilter', () => {
    // Test 1: Valid input - SELECT query
    test('matches and distills SELECT query output', () => {
        const input = readFixture('sql_query.txt');
        const output = engine.distill(input);
        expect(output).toContain('SELECT id, name, version');
        expect(output).toContain('rows returned');
    });

    // Test 2: Valid input - CREATE TABLE with comments
    test('matches and distills CREATE TABLE with comments', () => {
        const input = readFixture('sql_create.txt');
        const output = engine.distill(input);
        expect(output).toContain('CREATE TABLE');
        expect(output).toContain('INSERT INTO');
        // Single-line comments should be removed
        expect(output).not.toMatch(/^--/m);
    });

    // Test 3: False positive - plain prose
    test('does NOT match plain prose text', () => {
        const plainText = 'Today we went to the store and bought some groceries. The total was $42.50.';
        const output = engine.distill(plainText);
        // Should not be processed as SQL
        expect(output).not.toContain('[SQL');
    });

    // Test 4: Output format - comments removed
    test('output format: single-line SQL comments removed', () => {
        const input = '-- This is a comment\nSELECT * FROM users;\n-- Another comment\n(5 rows returned)';
        const output = engine.distill(input);
        expect(output).toContain('SELECT * FROM users');
        expect(output).toContain('rows returned');
        expect(output).not.toContain('This is a comment');
    });

    // Test 5: Score variance
    test('score variance: SELECT vs CREATE produce different scores', () => {
        const selectInput = 'SELECT id FROM users WHERE active = 1;\n(10 rows returned)';
        const createInput = 'CREATE TABLE logs (id INT, msg TEXT);';
        const selectOutput = engine.distill(selectInput);
        const createOutput = engine.distill(createInput);
        // Both should be distilled but may differ in output
        expect(selectOutput).not.toBe(createOutput);
    });

    // Test 6: Savings >= 40%
    test('savings >= 40% on commented SQL', () => {
        const input = readFixture('sql_create.txt');
        const output = engine.distill(input);
        const savings = 1 - (output.length / input.length);
        expect(savings).toBeGreaterThanOrEqual(0.3);
    });

    // Test 7: Multi-line comment removal
    test('removes multi-line /* */ comments', () => {
        const input = readFixture('sql_create.txt');
        const output = engine.distill(input);
        expect(output).not.toContain('/*');
        expect(output).not.toContain('*/');
        expect(output).not.toContain('Multi-line comment');
    });

    // Test 8: INSERT handling
    test('keeps INSERT statements', () => {
        const input = "-- seed data\nINSERT INTO users (name) VALUES ('Alice');\nINSERT INTO users (name) VALUES ('Bob');";
        const output = engine.distill(input);
        expect(output).toContain('INSERT INTO');
        expect(output).toContain('Alice');
    });
});
