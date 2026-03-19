import test from 'node:test';
import assert from 'node:assert';
import { createOmniEngine, readFixture } from '../test-helper.js';

test('Node Filter - NPM Install', async () => {
    const engine = await createOmniEngine();
    const input = readFixture('npm_install.txt');
    const output = engine.distill(input);
    
    // Node filter keeps the summary lines
    assert.match(output, /added 154 packages/);
    assert.match(output, /audited 155 packages/);
});
