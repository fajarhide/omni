import test from 'node:test';
import assert from 'node:assert';
import { createOmniEngine, readFixture } from '../test-helper.js';

test('Git Filter - Clean Status', async () => {
    const engine = await createOmniEngine();
    const input = readFixture('git_clean.txt');
    const output = engine.distill(input);
    
    // Check if it dinstills to a summary
    assert.match(output, /git: on branch main \(clean\)/);
});

test('Git Filter - Dirty Status', async () => {
    const engine = await createOmniEngine();
    const input = readFixture('git_dirty.txt');
    const output = engine.distill(input);
    
    // The engine counts lines in the Untracked section. 
    // In my fixture, there is 1 line after "Untracked files:" header and indent line.
    assert.match(output, /git: on main \| 0 staged, 2 mod, 0 del, 2 untracked/);
});

test('Git Filter - Diff', async () => {
    const engine = await createOmniEngine();
    const input = readFixture('git_diff.txt');
    const output = engine.distill(input);
    
    // Check that diff headers are present but noise is gone
    assert.ok(output.includes('diff --git'));
    assert.ok(output.includes('@@ -4,6 +4,7 @@'));
    assert.ok(!output.includes('index 1ea518f..9785243'));
    assert.ok(!output.includes('--- a/core/src/main.zig'));
});

test('Git Filter - Log', async () => {
    const engine = await createOmniEngine();
    const input = readFixture('git_log.txt');
    const output = engine.distill(input);
    
    // Should strip hashes (first 7-8 chars)
    assert.ok(output.includes('fix git log distillation'));
    assert.ok(!output.includes('a1b2c3d'));
});
