import test from 'node:test';
import assert from 'node:assert';
import { createOmniEngine, readFixture } from '../test-helper.js';

test('Docker Filter - Build Output', async () => {
    const engine = await createOmniEngine();
    const input = readFixture('docker_build.txt');
    const output = engine.distill(input);
    
    // Docker filter keeps Step lines and success markers
    assert.match(output, /Step 1\/5 : FROM alpine:latest/);
    assert.match(output, /Successfully built b4c9e2c71c4c/);
});
