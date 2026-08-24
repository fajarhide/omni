#!/usr/bin/env bash
# #704. Replay the frozen corpus and record the result as a file in the repo.
#
# Every published figure before this was measured on `execution_traces`, which
# `TRACE_RETENTION_DAYS` deletes after 7 days. Two releases therefore reported
# numbers from two different corpora, and the delta between them mixed a code
# change with a corpus change. Nobody could say which.
#
# The corpus lives in `bench-corpus/` and is local: it holds real command output
# and is in `.git/info/exclude`. What lands in the repo is the measurement and a
# summary of what produced it, so a reader can see the composition and verify the
# corpus did not change between two releases, without ever holding the payloads.
#
#   docs/benchmarks/<version>.json   the measurement, diffable across releases
#
# Build the corpus first, once:
#   python3 docs/internal/runbooks/build-bench-corpus.py
#
# Then:
#   make bench
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CORPUS="$ROOT/bench-corpus/traces.jsonl"
MANIFEST="$ROOT/bench-corpus/manifest.json"
OUT_DIR="$ROOT/docs/benchmarks"

if [ ! -f "$CORPUS" ]; then
  echo "no frozen corpus at $CORPUS" >&2
  echo "build it: python3 docs/internal/runbooks/build-bench-corpus.py" >&2
  exit 1
fi

VERSION="$(grep -m1 '^version' "$ROOT/Cargo.toml" | cut -d'"' -f2)"
COMMIT="$(git -C "$ROOT" rev-parse --short HEAD)"
# Greptile on #704. The artifact records HEAD, so a run over a modified tree would
# claim it measured that commit. Recording the fact is enough; refusing would block
# the ordinary case of measuring a change before committing it.
if [ -n "$(git -C "$ROOT" status --porcelain)" ]; then DIRTY=true; else DIRTY=false; fi
LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT

echo "replaying $(wc -l < "$CORPUS" | tr -d ' ') traces against $VERSION ($COMMIT)"
OMNI_BENCH_CORPUS="$CORPUS" \
  cargo test --release --test bench_replay -- --ignored --nocapture >"$LOG" 2>&1 || {
  tail -30 "$LOG" >&2
  exit 1
}

mkdir -p "$OUT_DIR"
# The figures come out of the replay's own output rather than being recomputed
# here, so this script cannot disagree with the harness it ran. A missing field
# is left null instead of defaulted: a benchmark that reports 0 where it failed
# to read is the failure mode this whole ticket is about.
python3 - "$LOG" "$MANIFEST" "$VERSION" "$COMMIT" "$OUT_DIR" "$DIRTY" <<'PY'
import json, re, sys

log, manifest_path, version, commit, out_dir, dirty = sys.argv[1:7]
text = open(log, errors="replace").read()
manifest = json.load(open(manifest_path))

# Greptile on #704. The report copies the manifest's `corpus_sha256`, so the
# manifest has to be proven to describe the file the replay just read. Without
# this, editing or rebuilding `traces.jsonl` without its manifest produces two
# measurements over different inputs carrying one identity, and their delta reads
# as a code change. That is the defect this whole ticket exists to remove, so it
# aborts rather than warns.
import hashlib

corpus_path = manifest_path.replace("manifest.json", "traces.jsonl")
with open(corpus_path, "rb") as fh:
    actual = hashlib.sha256(fh.read()).hexdigest()
expected = manifest.get("traces_sha256")
if expected is None:
    sys.exit(
        "manifest has no traces_sha256; rebuild it with "
        "docs/internal/runbooks/build-bench-corpus.py"
    )
if actual != expected:
    sys.exit(
        f"corpus does not match its manifest\n"
        f"  traces.jsonl {actual}\n"
        f"  manifest     {expected}\n"
        "rebuild the manifest before measuring anything"
    )

# And `corpus_sha256` itself, because that is the field the report publishes as
# the corpus identity. Checking only the payload file leaves a manifest whose
# entries or whose stated hash were edited independently, and the artifact would
# then carry a hash that describes neither. Recomputed the way the builder
# computes it, so the two cannot drift.
restated = hashlib.sha256(
    json.dumps(manifest["entries"], sort_keys=True).encode()
).hexdigest()
if restated != manifest.get("corpus_sha256"):
    sys.exit(
        f"manifest does not describe itself\n"
        f"  entries hash to {restated}\n"
        f"  corpus_sha256   {manifest.get('corpus_sha256')}\n"
        "rebuild the manifest before measuring anything"
    )


def num(pattern):
    m = re.search(pattern, text)
    return float(m.group(1)) if m else None


def whole(pattern):
    m = re.search(pattern, text)
    return int(m.group(1)) if m else None


def by_class():
    """The per-class table the replay prints, parsed rather than recomputed.

    Class names come from `trace_class` in the harness, which is a fixed set of
    generic buckets (`file read`, `search`, `git`, `infra`, `build and test`,
    `other`), so unlike command classes they carry nothing local and need no
    allowlist. Rows are read by shape; a table that stops printing a column shows
    up as an empty dict and fails the missing-field check below.
    """
    block = re.search(
        r"by command class.*?\n(.*?)\nledger arm:", text, re.S
    )
    if not block:
        return {}
    out = {}
    for line in block.group(1).splitlines():
        m = re.match(
            r"^(\S.*?)\s{2,}(\d+)\s+(\d+)\s+([\d.]+)%\s+([\d.]+)%"
            r"\s+([\d.]+)%\s+([\d.]+)%",
            line,
        )
        if not m:
            continue
        out[m.group(1).strip()] = {
            "calls": int(m.group(2)),
            "input_bytes": int(m.group(3)),
            "filters_pct": float(m.group(4)),
            "with_ledger_pct": float(m.group(5)),
            "repetition_available_pct": float(m.group(6)),
            "capture_rate_pct": float(m.group(7)),
        }
    return out


# A command class is the basename of whatever ran, so the corpus knows the names
# of local scripts, and the first run of this put a client's name into the
# artifact. Only names on this list are published; everything else is summed into
# `[other]`.
#
# **An allowlist, and deliberately not a denylist.** A denylist would have to
# carry the names it protects against, which means committing the whole client
# list to a public repository in one file, in git history, forever. That is a
# larger leak than the one it prevents. An allowlist can only under-disclose, and
# under-disclosing here costs nothing: the classes worth reporting are the generic
# tools any reader would recognise, and a name nobody outside this machine can
# interpret was never disclosure in the first place.
#
# Adding a name is a deliberate act. If a real tool keeps landing in `[other]`,
# put it here rather than reaching for a pattern that admits whole shapes.
PUBLISHABLE = set(
    """
    apt awk az bash bun bundle cargo cat cd chmod cp curl cut deno df diff dig
    docker du echo env find gh git go gradle grep head helm httpie ip jq kubectl
    ls make mkdir mv nc netstat next nmap node npm npx openssl pip pnpm ps psql
    pytest python python3 rg rm rsync ruby sed seq sh sort ssh sudo tail tar tee
    terraform terragrunt tr uname uniq uv vercel wc wget xargs yarn yq zsh
    """.split()
)

CLASS_FLOOR = 0.005
corpus_bytes = manifest["bytes"] or 1
named, other = {}, 0
for cls, b in manifest["class_bytes"].items():
    if cls in PUBLISHABLE and b / corpus_bytes >= CLASS_FLOOR:
        named[cls] = b
    else:
        other += b
if other:
    named["[other]"] = other

report = {
    "version": version,
    "commit": commit,
    # True means the working tree carried uncommitted changes, so `commit` names
    # the nearest commit and not the source that was measured.
    "dirty_tree": dirty == "true",
    "corpus": {
        "sha256": manifest["corpus_sha256"],
        "traces": manifest["traces"],
        "bytes": manifest["bytes"],
        "sessions": manifest["sessions"],
        # Byte share per command class, so a reader can see what the corpus is
        # made of and judge whether a figure generalises to their own work. Only
        # classes at or above CLASS_FLOOR are named; see above for why.
        "class_bytes": named,
        "class_floor": CLASS_FLOOR,
        "buckets": manifest["buckets"],
    },
    "result": {
        "net_savings_pct": num(r"NET SAVINGS:\s+([\d.]+)%"),
        "traces_replayed": whole(r"corpus:\s+(\d+) traces"),
        "repeated_bytes_pct": num(r"repeated bytes handed to the ledger: \d+ \(([\d.]+)%"),
        "ledger_claimed_pct": num(r"claimed by the ledger:\s+\d+ \(([\d.]+)%"),
        # #708. Per class, and `captured` is the one that does not move with the
        # workload: on this corpus file reads save 4.5% where a week of large
        # repeated reads read 89.6%, while the share of available repetition the
        # ledger takes stays in a narrow band. A savings percentage describes the
        # week; this describes OMNI.
        "by_class": by_class(),
    },
}
missing = [k for k, v in report["result"].items() if v is None or v == {}]
if missing:
    sys.exit(f"replay output did not carry: {', '.join(missing)}")

path = f"{out_dir}/{version}.json"
with open(path, "w") as fh:
    json.dump(report, fh, indent=2, sort_keys=True)
    fh.write("\n")
print(f"\nwrote {path}")
for k, v in report["result"].items():
    print(f"  {k:<22}{v}")
print(f"  corpus_sha256         {report['corpus']['sha256'][:16]}...")
PY
