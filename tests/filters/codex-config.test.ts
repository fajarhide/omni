import { beforeAll, describe, expect, test } from 'bun:test';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { createOmniEngine } from '../test-helper.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const configPath = path.join(__dirname, '../../omni_config.json');
const config = JSON.parse(fs.readFileSync(configPath, 'utf-8'));

let engine: { distill: (text: string) => string };

beforeAll(async () => {
    engine = await createOmniEngine(config);
});

describe('Codex project filters', () => {
    test('distills TypeScript compiler errors into a compact summary', () => {
        const input = [
            "src/index.ts(3,12): error TS2304: Cannot find name 'foo'.",
            "src/app.ts(8,4): error TS2322: Type 'number' is not assignable to type 'string'."
        ].join('\n');

        const output = engine.distill(input);

        expect(output).toBe("tsc: 2 diagnostics | last TS2322: Type 'number' is not assignable to type 'string'.");
    });

    test('distills ESLint plural summary lines', () => {
        const input = [
            "/tmp/example.ts",
            "  10:5  error    Unexpected any. Specify a different type  @typescript-eslint/no-explicit-any",
            "  12:1  warning  Missing return type on function          @typescript-eslint/explicit-function-return-type",
            "",
            "✖ 3 problems (2 errors, 2 warnings)"
        ].join('\n');

        const output = engine.distill(input);

        expect(output).toBe('eslint: 3 problems | 2 errors | 2 warnings');
    });

    test('distills failing Jest summaries', () => {
        const input = 'Tests:       1 failed, 9 passed, 10 total';
        const output = engine.distill(input);

        expect(output).toContain('jest: 9 passed | 1 failed | 10 total');
    });

    test('distills passing Vitest summaries', () => {
        const input = [
            'Test Files 2 passed (2)',
            'Tests 5 passed (5)'
        ].join('\n');

        const output = engine.distill(input);

        expect(output).toBe('vitest: 2 files passed | 5 tests passed');
    });
});
