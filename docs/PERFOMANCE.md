# OMNI — Performance & Capabilities Showcase

![OMNI Performance Dashboard](https://omni.weekndlabs.com/media/performance.png)

---

## Key Performance Metrics

### Hero Stats (for headlines)

| Metric | Value | Context |
|--------|-------|---------|
| **Peak Noise Reduction** | **99.5%** | Docker build output: 9.2KB → 49 bytes |
| **Pipeline Latency** | **< 100ms** | End-to-end on release binary |
| **All-Time Token Savings** | **97.3%** | From real production usage (232 commands) |
| **Estimated Cost Saved** | **$35+ USD** | From single developer usage |
| **Test Coverage** | **398 tests** | Zero failures, zero ignored |
| **Binary Size** | **8.4 MB** | Native Rust, zero runtime deps |

### Savings by Command Type (for bar charts / infographics)

| Command | Savings | Best For |
|---------|---------|----------|
| Docker Build (noise) | **99.5%** | Eliminates "Step X/Y", "Sending context", pulling layers |
| Docker Build (layered) | **90%** | Strips CACHED/FROM/RUN noise, keeps only errors |
| Test Output (pytest) | **78%** | Keeps only failed tests + stack traces |
| Git Status | **77%** | Removes clean file listings, keeps only changes |
| Kubernetes | **62%** | Strips healthy pod rows, surfaces errors |
| Git Diff | **55%** | Preserves hunks with changes, drops context noise |

---

## Publishable Use Cases

### Use Case 1: "The Expensive `npm install`"

**Problem**: Agent runs `npm install` — 500 lines of resolved dependencies, audit warnings, and progress bars flood the context.

**Without OMNI**:
```
added 847 packages, and audited 848 packages in 12s
npm warn deprecated inflight@1.0.6: This module is not supported...
npm warn deprecated glob@7.2.0: Glob versions prior to v9 are...
npm warn deprecated rimraf@3.0.2: Rimraf versions prior to v4...
... (500+ more lines of progress and deprecation warnings)
```

**With OMNI**:
```
npm install: 847 packages, 12s. 3 deprecation warnings (non-critical).
```

**Savings**: ~95% token reduction. AI focuses on actual errors, not noise.

---

### Use Case 2: "The Giant `cargo test`"

**Problem**: Cargo test suite dumps 500+ "test X ... ok" lines. The 3 actual failures get buried.

**Without OMNI**: 16,515 bytes of output. AI struggles to find the needle.

**With OMNI**: Only failed test names + error messages + stack traces. AI immediately knows what to fix.

**Savings**: 78%+ — AI response quality improves dramatically.

---

### Use Case 3: "The Docker Build from Hell"

**Problem**: Building a multi-stage Dockerfile produces thousands of lines of layer caching, dependency downloading, and compilation noise.

**Without OMNI**: 9,207 bytes of "Sending build context", "Step 1/20", "Removing intermediate container"...

**With OMNI**: 49 bytes — just the final status or error.

**Savings**: **99.5%** — this is OMNI's signature benchmark.

---

### Use Case 4: "The Kubernetes Debug Loop"

**Problem**: `kubectl get pods` returns 50 pods. 47 are Running fine. Agent needs to find the 3 that are CrashLoopBackOff.

**Without OMNI**: Full pod table with all 50 rows of STATUS, RESTARTS, AGE.

**With OMNI**: Only the 3 problematic pods + their error status.

**Savings**: 62% — but more importantly, **zero diagnostic confusion**.

---

### Use Case 5: "The Git Diff Review"

**Problem**: `git diff` shows 200+ lines across 10 files. Agent re-reads unchanged context lines.

**Without OMNI**: Full unified diff with 3-line context padding per hunk.

**With OMNI**: Only changed lines + minimal context. Hunks intelligently scored by relevance.

**Savings**: 55% — AI reviews changes faster with higher accuracy.

---

### Use Case 6: "Session Intelligence — The Redundant Read"

**Problem**: Agent reads `src/main.rs` for the 4th time in one session. It already has the content in context.

**Without OMNI**: Full file content dumped again → wastes 3,000+ tokens.

**With OMNI**: 
```
OMNI Guard: Redundant read detected for src/main.rs. 
It has been accessed 4x. The file is likely already in context.
```

**Savings**: 100% of wasted re-read tokens. AI stays focused.

---

## Feature Highlights

### For Developers Who Care About Costs

> **"OMNI saved me $35 in one month of casual use. For teams running multiple agents, that's hundreds per month."**

- Real-time cost tracking via `omni stats`
- Per-agent savings breakdown (Claude, Cursor, Antigravity, Aider)
- Model-aware pricing (Claude Sonnet, GPT-4o pricing tables built-in)

### For Developers Who Care About AI Quality

> **"My AI stopped hallucinating after I installed OMNI. It finally focuses on the actual error instead of getting confused by 500 lines of dependency logs."**

- Semantic signal scoring: Critical > Important > Context > Noise
- Session-aware boosting: errors and hot files get priority
- Anti-hallucination guards: factual warnings only, zero speculation

### For Team Leads / Engineering Managers

> **"We run 5 agents across 3 repos. OMNI keeps them coordinated and efficient."**

- Multi-agent awareness (`omni_agents`)
- Cross-session knowledge persistence (`omni_knowledge`)
- Per-agent tuning via `config.toml`
- Works with Claude Code, Cursor, Antigravity, Aider, OpenCode, Codex, Hermes, Pi Agent


## Real Production Stats

This is a real output from `omni stats` :

```
─────────────────────────────────────────────────
 OMNI Signal Report
─────────────────────────────────────────────────
  Today:        21 commands │  42K → 24K  tokens │  41.8% saved │ ~$0.05 USD
  This Week:    38 commands │  48K → 27K  tokens │  43.6% saved │ ~$0.06 USD
  All Time:    232 commands │ 13.0M → 346K tokens │  97.3% saved │ ~$35.94 USD

  Top Commands:
    find .             ████████████████████  100.0%  ( 3x)  (-12.6M tokens)
    cat /Users/faja... ████████████████████   98.0%  ( 2x)  (-14K tokens)
    docker build       ███████████░░░░░░░░░   56.4%  ( 4x)  (-11K tokens)
    git diff           ██████████░░░░░░░░░░   49.2%  (50x)  (-4K tokens)

  Agent Distribution:
    Claude Code        ████████░░░░░░░░░░░░   39.2%  (91x)   46.2% saved
    aider              ██░░░░░░░░░░░░░░░░░░    9.1%  (21x)   33.6% saved
    Cursor AI          ██░░░░░░░░░░░░░░░░░░    8.6%  (20x)   22.6% saved
    Antigravity        █░░░░░░░░░░░░░░░░░░░    2.6%  ( 6x)    0.3% saved
─────────────────────────────────────────────────
```
