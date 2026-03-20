import test from 'node:test';
import assert from 'node:assert';
import { createOmniEngine, readFixture } from '../test-helper.js';

test('Cat Filter - Markdown Document', async () => {
    const engine = await createOmniEngine();
    const input = readFixture('cat_readme.txt');
    const output = engine.distill(input);
    
    // Cat filter extracts headers and list items
    assert.match(output, /# OMNI Project/);
    assert.match(output, /## Features/);
    assert.match(output, /### Getting Started/);
    assert.match(output, /- Fast distillation/);
    
    // It should NOT include the regular paragraph text if it's long, 
    // but here it might include it if it's short. 
    // Our implementation keeps headers and short list items.
    // For high-confidence docs, it returns clean headers without manifest prefix.
});

test('Cat Filter - Raw Content Summary', async () => {
    const engine = await createOmniEngine();
    const input = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7\nLine 8\nLine 9\nLine 10\n" +
                  "Line 11\nLine 12\nLine 13\nLine 14\nLine 15\nLine 16\nLine 17\nLine 18\nLine 19\nLine 20";
    const output = engine.distill(input);
    
    // Since there are no headers/lists, it should return a summary
    assert.match(output, /cat distilled/);
});
