# Agent Instructions

This repository is optimized for OMNI-assisted workflows.

## OMNI Directive

When working in this project, prefer OMNI tools whenever they improve signal density without hiding detail that matters.

Use these first for noisy or high-volume interactions:
- `omni_execute` for shell commands that produce long or repetitive output
- `omni_read_file` for large files, logs, SQL, JSON, and generated artifacts
- `omni_view_file` for targeted file ranges when the file is long
- `omni_list_dir`, `omni_find_by_name`, and `omni_grep_search` for repository discovery

Use raw shell/file reads when exact verbatim output is more important than compression:
- short files
- precise metadata inspection
- tiny command outputs such as `pwd`, `ls`, or a small `sed` range
- any case where filtering could hide a detail under investigation

## Practical Defaults

Prefer OMNI for commands such as:
- `git diff`
- `git status`
- `docker build`
- `npm install`
- `pnpm install`
- `bun test`
- `pytest`
- `cargo test`
- `go test`
- long build logs

Prefer raw commands for:
- `sed -n '1,80p' package.json`
- `ls -1 tests`
- one-off inspection of short files

## Decision Rule

If the output is likely noisy, repetitive, or longer than a short screenful, use OMNI first.
If the task requires exact text fidelity, use raw output first.

## Local Config

This repository contains a local `omni_config.json`. Trust it with `omni_trust` when you want OMNI to apply project-local filters in addition to the global configuration.
