import fs from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';
import { WASI } from 'wasi';
import { argv, env } from 'process';

const __dirname = dirname(fileURLToPath(import.meta.url));
const wasmPath = join(__dirname, '../core/zig-out/bin/omni-wasm.wasm');
const wasmBuffer = fs.readFileSync(wasmPath);

// Detect runtime: bun vs node
const isBun = typeof globalThis.Bun !== 'undefined';

export async function createOmniEngine(config = null) {
    const wasi = new WASI({
        args: argv,
        env,
        version: 'preview1',
        preopens: { '.': '.' }
    });

    if (isBun) {
        // Patch proc_exit BEFORE creating import object.
        // This way WASI still registers the instance properly via start(),
        // but proc_exit(0) from _start won't kill the bun process.
        const origProcExit = wasi.wasiImport.proc_exit;
        wasi.wasiImport.proc_exit = (code) => {
            // Swallow proc_exit(0) — it's _start finishing normally.
            // Throw a sentinel to stop _start execution.
            throw new Error('__wasi_proc_exit_' + code);
        };
    }

    const importObject = { wasi_snapshot_preview1: wasi.wasiImport };
    const { instance } = await WebAssembly.instantiate(wasmBuffer, importObject);

    if (isBun) {
        try {
            wasi.start(instance);
        } catch (e) {
            // Catch the sentinel from our patched proc_exit
            if (!e.message || !e.message.startsWith('__wasi_proc_exit_')) {
                throw e;
            }
        }
    } else {
        wasi.start(instance);
    }

    const { alloc, free, compress, init_engine_with_config, memory } = instance.exports;

    function writeString(str) {
        const bytes = Buffer.from(str);
        const ptr = alloc(bytes.length);
        const mem = new Uint8Array(memory.buffer);
        mem.set(bytes, ptr);
        return { ptr, len: bytes.length };
    }

    function readString(u64) {
        const len = Number(u64 >> 32n);
        const ptr = Number(u64 & 0xFFFFFFFFn);
        const bytes = new Uint8Array(memory.buffer, ptr, len);
        return Buffer.from(bytes).toString();
    }

    if (config) {
        const { ptr, len } = writeString(JSON.stringify(config));
        init_engine_with_config(ptr, len);
        free(ptr, len);
    }

    return {
        distill: (text) => {
            const { ptr, len } = writeString(text);
            const resultPtr = compress(ptr, len);
            const output = readString(resultPtr);
            free(ptr, len);
            return output;
        }
    };
}

export function readFixture(name) {
    return fs.readFileSync(join(__dirname, 'fixtures', name), 'utf-8');
}
