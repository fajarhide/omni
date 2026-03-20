import fs from 'fs';
import { WASI } from 'wasi';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const WASM_PATH = path.join(__dirname, '../core/zig-out/bin/omni-wasm.wasm');

async function test() {
    console.log("Testing OMNI Discovery Engine...");
    
    const wasi = new WASI({
        version: 'preview1',
        args: [],
        env: process.env,
        preopens: { '.': path.dirname(WASM_PATH) }
    });

    const wasmBuffer = fs.readFileSync(WASM_PATH);
    const { instance } = await WebAssembly.instantiate(wasmBuffer, {
        wasi_snapshot_preview1: wasi.wasiImport,
    });

    wasi.start(instance);
    const exports = instance.exports;

    const noisyInput = `
Removing intermediate container 1234567890
Removing intermediate container abcdef1234
Removing intermediate container 9876543210
Removing intermediate container 0123456789
Step 1/5 : FROM node:20
Step 2/5 : COPY . .
Step 3/5 : RUN npm install
Removing intermediate container ffeeddccbb
    `.trim();

    const encoder = new TextEncoder();
    const inputBytes = encoder.encode(noisyInput);
    const inputPtr = exports.alloc(inputBytes.length);
    
    const memView = new Uint8Array(exports.memory.buffer);
    memView.set(inputBytes, inputPtr);

    console.log("Calling discover()...");
    const resultRaw = exports.discover(inputPtr, inputBytes.length);
    const resultPtr = Number(BigInt(resultRaw) & 0xFFFFFFFFn);
    const resultLen = Number(BigInt(resultRaw) >> 32n);

    const outputBytes = new Uint8Array(exports.memory.buffer, resultPtr, resultLen);
    const resultJson = new TextDecoder().decode(outputBytes);
    
    console.log("Result JSON:", resultJson);
    const candidates = JSON.parse(resultJson);

    if (candidates.length > 0) {
        console.log(`\n✅ Success: Found ${candidates.length} candidates.`);
        candidates.forEach((c, i) => {
            console.log(`${i+1}. [${c.action}] ${c.name} (Conf: ${c.confidence})`);
            console.log(`   Trigger: ${c.trigger}`);
        });
    } else {
        console.log("\n❌ Failure: No candidates found.");
        process.exit(1);
    }
}

test().catch(e => {
    console.error(e);
    process.exit(1);
});
