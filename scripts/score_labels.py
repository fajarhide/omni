#!/usr/bin/env python3
"""Precision and recall for the Critical tier, against a labelled sample (#818).

    scripts/score_labels.py sample.jsonl labels.jsonl [baseline.json]

`sample.jsonl` comes from `cargo run --example sample_blocks`, one row per block
with the tier `semantic::classify_block` assigned. `labels.jsonl` carries the same
`trace`, `seq` and `block` keys with a `label` of `critical` or `not_critical`.

The number that matters is the miss: a block a labeller called critical that the
pipeline tiered below Important is a line the distiller may drop, and that costs
more than the compression ever saved. Precision is reported beside it so a fix
that tiers everything Critical cannot pass as an improvement.

Exits 1 when the miss rate is worse than the baseline, so the bench can gate on
it. With no baseline it prints the numbers and exits 0, which is how the first
one is produced.
"""

import json
import sys
from pathlib import Path

KEY = ("trace", "seq", "block")
KEPT = {"Critical", "Important"}


def rows(path):
    for line in Path(path).read_text().splitlines():
        if line.strip():
            yield json.loads(line)


def score(sample_path, labels_path):
    tiers = {tuple(r[k] for k in KEY): r["tier"] for r in rows(sample_path)}
    tp = fp = fn = judged = 0
    for r in rows(labels_path):
        key = tuple(r[k] for k in KEY)
        tier = tiers.get(key)
        if tier is None:
            continue
        judged += 1
        called_critical = tier in KEPT
        is_critical = r["label"] == "critical"
        tp += called_critical and is_critical
        fp += called_critical and not is_critical
        fn += not called_critical and is_critical
    return {
        "judged": judged,
        "true_positive": tp,
        "false_positive": fp,
        "missed": fn,
        "precision": tp / (tp + fp) if tp + fp else 0.0,
        "recall": tp / (tp + fn) if tp + fn else 0.0,
        "miss_rate": fn / (tp + fn) if tp + fn else 0.0,
    }


def main(argv):
    if len(argv) < 3:
        print(__doc__)
        return 2
    result = score(argv[1], argv[2])
    for k, v in result.items():
        print(f"{k:>16}: {v:.4f}" if isinstance(v, float) else f"{k:>16}: {v}")
    if len(argv) < 4:
        return 0
    baseline = json.loads(Path(argv[3]).read_text())
    was, now = baseline["miss_rate"], result["miss_rate"]
    if now > was:
        print(f"\nmiss rate regressed: {was:.4f} -> {now:.4f}")
        return 1
    print(f"\nmiss rate held: {was:.4f} -> {now:.4f}")
    return 0


def self_test():
    """One runnable check: a miss is counted, and a Context tier is the miss."""
    import tempfile

    with tempfile.TemporaryDirectory() as d:
        sample = Path(d, "s.jsonl")
        labels = Path(d, "l.jsonl")
        sample.write_text(
            '{"trace":1,"seq":0,"block":0,"tier":"Critical"}\n'
            '{"trace":1,"seq":0,"block":1,"tier":"Context"}\n'
            '{"trace":1,"seq":0,"block":2,"tier":"Important"}\n'
        )
        labels.write_text(
            '{"trace":1,"seq":0,"block":0,"label":"critical"}\n'
            '{"trace":1,"seq":0,"block":1,"label":"critical"}\n'
            '{"trace":1,"seq":0,"block":2,"label":"not_critical"}\n'
        )
        got = score(sample, labels)
        assert got["missed"] == 1, got
        assert got["false_positive"] == 1, got
        assert abs(got["recall"] - 0.5) < 1e-9, got
        assert abs(got["precision"] - 0.5) < 1e-9, got
        assert abs(got["miss_rate"] - 0.5) < 1e-9, got
    print("self test: a Context block a labeller called critical counts as a miss")


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--self-test":
        self_test()
    else:
        sys.exit(main(sys.argv))
