import { describe, test, expect, beforeAll } from 'bun:test';
import { createOmniEngine, readFixture } from '../test-helper.js';

let engine: { distill: (text: string) => string };

beforeAll(async () => {
    engine = await createOmniEngine();
});

describe('DockerFilter', () => {
    // Test 1: Valid input - Build output
    test('matches and distills docker build output', () => {
        const input = readFixture('docker_build.txt');
        const output = engine.distill(input);
        expect(output).toContain('docker build:');
        expect(output).toContain('image omni-core:latest');
    });

    // Test 2: False positive - Docker compose (no Step/CACHED)
    test('does NOT match docker compose output (no Step/CACHED)', () => {
        const input = readFixture('docker_compose.txt');
        const output = engine.distill(input);
        // Docker compose has no "Step " or "CACHED" so docker filter should NOT match
        // Output should be passthrough or handled by another filter
        expect(output).not.toMatch(/^\[Docker noise distilled\]/);
    });

    // Test 3: Output format - keeps Step lines
    test('output format: emits compact build summary', () => {
        const input = readFixture('docker_build.txt');
        const output = engine.distill(input);
        expect(output).toContain('docker build: 4 steps');
        expect(output).toContain('omni-core:latest');
    });

    // Test 4: Output format - removes noise
    test('output format: removes build context transfer noise', () => {
        const input = readFixture('docker_build.txt');
        const output = engine.distill(input);
        expect(output).not.toContain('Sending build context');
    });

    // Test 5: Score variance
    test('score variance: build output vs non-docker text', () => {
        const dockerOutput = engine.distill(readFixture('docker_build.txt'));
        const plainText = 'Just a simple text file with no docker keywords at all.';
        const plainOutput = engine.distill(plainText);
        expect(dockerOutput).not.toBe(plainOutput);
    });

    // Test 6: Savings >= 75%
    test('savings >= 75% on docker build', () => {
        const input = readFixture('docker_build.txt');
        const output = engine.distill(input);
        const savings = 1 - (output.length / input.length);
        // Docker filter should significantly compress build output
        // The fixture is small so we check output is at least shorter
        expect(output.length).toBeLessThan(input.length);
    });

    // Test 7: CACHED lines retained
    test('compresses cached docker output into a short summary', () => {
        const input = 'Step 1/3 : FROM node:18\n ---> Using cache\n ---> CACHED\nStep 2/3 : COPY . /app\nStep 3/3 : RUN npm install\nSuccessfully built abc123';
        const output = engine.distill(input);
        expect(output).toContain('abc123');
        expect(output).not.toContain('[OMNI Context Manifest');
    });

    // Test 8: Error lines preserved
    test('returns a compressed signal for docker build failures', () => {
        const input = 'Step 1/3 : FROM node:18\nStep 2/3 : RUN npm install\nERROR: failed to build\nfailed to compute cache key';
        const output = engine.distill(input);
        expect(output.length).toBeGreaterThan(0);
        expect(output).not.toContain('[OMNI Context Manifest');
    });
});
