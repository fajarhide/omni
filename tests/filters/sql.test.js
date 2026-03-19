import test from 'node:test';
import assert from 'node:assert';
import { createOmniEngine, readFixture } from '../test-helper.js';

test('SQL Filter - Select Query', async () => {
    const engine = await createOmniEngine();
    const input = readFixture('sql_query.txt');
    const output = engine.distill(input);
    
    // SQL filter removes comments and extra whitespace but keeps the query and result indicator
    assert.match(output, /SELECT id, name, version/);
    assert.match(output, /rows returned/);
});
