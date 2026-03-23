# OMNI Learn — Automatic Pattern Discovery

OMNI can automatically generate TOML filters by analyzing repetitive noise in your terminal output. This is the fastest way to shrink your token usage without writing regex manually.

## How it works

The `omni learn` engine uses a "Word Prefix Frequency" algorithm:
1. It splits incoming text into lines.
2. It extracts the **first 3 words** of every line as a signature.
3. If a signature appears **3 or more times**, OMNI identifies it as a "Repetitive Noise Candidate".
4. It then generates a TOML filter that can either **Strip** (remove the line) or **Count** (replace with a summary count).

## 1. Passive Background Learning

Whenever OMNI runs as a hook (e.g., inside Claude Code), it silently monitors for patterns. If it finds noise that isn't already covered by a filter, it records it in a local queue:

`~/.omni/learn_queue.jsonl`

You can process this queue at any time:
```bash
omni learn --from-queue --dry-run
```

## 2. Manual Learning (Pipe Mode)

If you have a log file or a command output that is very noisy, you can pipe it directly into OMNI to generate a filter:

```bash
cat build.log | omni learn --dry-run
```

## 3. Applying Learned Filters

Once you are happy with the dry-run output, apply it:

```bash
cat build.log | omni learn --apply
```

This will:
- Create (or append to) `~/.omni/filters/learned.toml`.
- Assign a unique ID to the filter (e.g., `learned_1711234567`).
- Include **inline tests** based on the actual log data to ensure the filter works as expected.

## Best Practices

- **Validate First**: Always use `--dry-run` before `--apply`.
- **Review `learned.toml`**: Since it's a standard TOML file, you can manually edit descriptions or refine regex patterns later.
- **Run Diagnostics**: After applying, run `omni doctor` to ensure the new filters are loaded correctly.
- **Verify Tests**: Run `omni learn --verify` to execute all inline tests in your filter library.

---
> [!TIP]
> OMNI Learn is designed to be conservative. It only suggests filters for patterns it is very confident in.
