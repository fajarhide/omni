# TOML Filter Writing Guide

OMNI supports user-defined TOML filters for distilling output from any command — including internal company tools, custom deploy scripts, and CI pipelines.

| Directory | Priority | Description |
|---|---|---|
| **Built-in (embedded)** | Lowest | Compiled into the OMNI binary |
| **User** (`~/.omni/filters/`) | Medium | Personal/User-global filters |
| **Project** (`.omni/filters/`) | Highest | Project-specific (requires `omni trust`) |

### Hierarchy Logic
1. **Project-local filters** override User filters.
2. **User filters** override Built-in filters.
3. **Built-in filters** are the fallback for standard commands.

> [!NOTE]
> **Built-in** filters are not visible in the filesystem because they are compiled into the binary (`embedded`). If you want to modify them, create a file with the same name in `~/.omni/filters/` to override their behavior.

## Basic Structure

```toml
schema_version = 1

[filters.my_filter]
description = "What this filter does"
match_command = "^my-tool\\b"           # Regex matching command output
strip_ansi = true                       # Remove ANSI color codes first
confidence = 0.85                       # Base confidence score (0.0-1.0)

# Output matchers (first match wins)
[[filters.my_filter.match_output]]
pattern = "Deployment successful"       # Regex match on output
message = "deploy: ✓ success"           # Replacement message
unless = "ROLLBACK"                     # Skip if this pattern also appears

# Line filters
strip_lines_matching = ["^\\[DEBUG\\]", "^Waiting"]
keep_lines_matching  = []               # Mutually exclusive with strip

max_lines = 30                          # Maximum output lines
on_empty = "my_filter: completed"       # Fallback if everything is stripped

# Inline tests
[[tests.my_filter]]
name = "strips debug lines"
input = """
[DEBUG] Connecting...
Deployment successful
"""
expected = "deploy: ✓ success"
```

## Real-World Examples

### 1. Company Deploy Tool

```toml
schema_version = 1

[filters.deploy]
description = "Internal deploy pipeline"
match_command = "^deploy\\b"
strip_ansi = true
confidence = 0.90

[[filters.deploy.match_output]]
pattern = "Successfully deployed to (\\w+)"
message = "deploy: ✓ $1"

[[filters.deploy.match_output]]
pattern = "ROLLBACK"
message = "deploy: ✗ ROLLBACK triggered"

strip_lines_matching = [
    "^\\[DEBUG\\]",
    "^Uploading artifact",
    "^Waiting for health check",
    "^\\s*\\d+%",
]
max_lines = 20
on_empty = "deploy: completed (no notable output)"

[[tests.deploy]]
name = "success case"
input = """
[DEBUG] Connecting to prod-cluster
Uploading artifact...
  45%
  90%
  100%
Successfully deployed to production
Health check passed
"""
expected = "deploy: ✓ production"

[[tests.deploy]]
name = "rollback case"
input = """
[DEBUG] Deploying v2.0
Health check FAILED
ROLLBACK initiated
"""
expected = "deploy: ✗ ROLLBACK triggered"
```

### 2. Internal Test Runner

```toml
schema_version = 1

[filters.itest]
description = "Internal integration test runner"
match_command = "^itest\\b"
strip_ansi = true

[[filters.itest.match_output]]
pattern = "(\\d+) passed, (\\d+) failed"
message = "itest: $1 passed, $2 failed"

strip_lines_matching = [
    "^Setting up",
    "^Tearing down",
    "^\\s+PASS ",
    "^Loading fixtures",
]
keep_lines_matching = ["FAIL", "Error", "Timeout"]
max_lines = 25

[[tests.itest]]
name = "summarizes test run"
input = """
Setting up test environment...
Loading fixtures...
  PASS  test_user_create
  PASS  test_user_login
  FAIL  test_user_delete - Timeout after 30s
Tearing down...
3 passed, 1 failed
"""
expected = "itest: 3 passed, 1 failed"
```

### 3. Database Migration Tool

```toml
schema_version = 1

[filters.dbmigrate]
description = "Database migration runner"
match_command = "^db-migrate\\b"
strip_ansi = true
confidence = 0.80

[[filters.dbmigrate.match_output]]
pattern = "Applied (\\d+) migration"
message = "db: $1 migrations applied"

strip_lines_matching = [
    "^Checking connection",
    "^Reading migration files",
    "^\\s+OK ",
]
keep_lines_matching = ["ERROR", "ROLLBACK", "already applied"]
max_lines = 15

[[tests.dbmigrate]]
name = "normal migration"
input = """
Checking connection to postgres://db:5432
Reading migration files...
  OK  001_create_users.sql
  OK  002_add_email_index.sql
Applied 2 migrations
"""
expected = "db: 2 migrations applied"
```

### 4. CI Pipeline Status

```toml
schema_version = 1

[filters.ci]
description = "CI/CD pipeline output"
match_command = "^ci-status\\b"
strip_ansi = true

[[filters.ci.match_output]]
pattern = "Pipeline (\\w+): (\\w+)"
message = "ci: $1 → $2"

strip_lines_matching = [
    "^Fetching",
    "^Polling",
    "^\\s+Stage \\d+/\\d+",
]
keep_lines_matching = ["failed", "error", "timeout", "cancelled"]
max_lines = 10

[[tests.ci]]
name = "pipeline success"
input = """
Fetching pipeline status...
Polling...
  Stage 1/4: Build (done)
  Stage 2/4: Test (done)
  Stage 3/4: Deploy (done)
  Stage 4/4: Verify (done)
Pipeline main: success
"""
expected = "ci: main → success"
```

### 5. Log Aggregator Query

```toml
schema_version = 1

[filters.logquery]
description = "Log aggregator query results"
match_command = "^logq\\b"
strip_ansi = true

strip_lines_matching = [
    "^Querying",
    "^Scanning \\d+ shards",
    "^Results from",
]
keep_lines_matching = ["ERROR", "WARN", "FATAL", "panic", "OOM"]
max_lines = 40
on_empty = "logq: no errors found"

[[tests.logquery]]
name = "filters to errors only"
input = """
Querying prod logs (last 1h)...
Scanning 42 shards...
Results from us-east-1:
2024-01-15 10:30:01 INFO  Request processed
2024-01-15 10:30:02 INFO  Request processed
2024-01-15 10:30:03 ERROR Connection timeout to redis-primary
2024-01-15 10:30:04 INFO  Request processed
"""
expected = "2024-01-15 10:30:03 ERROR Connection timeout to redis-primary"
```

## Filter Fields Reference

| Field | Type | Required | Description |
|---|---|---|---|
| `description` | String | No | Human-readable description |
| `match_command` | Regex | No | Match against command text |
| `strip_ansi` | Bool | No | Remove ANSI codes before processing |
| `confidence` | Float | No | Base confidence score (0.0-1.0) |
| `strip_lines_matching` | \[Regex\] | No | Lines matching these patterns are removed |
| `keep_lines_matching` | \[Regex\] | No | Only keep lines matching these (exclusive with strip) |
| `max_lines` | Int | No | Maximum output lines |
| `on_empty` | String | No | Fallback output if everything stripped |

### Match Output Fields

| Field | Type | Description |
|---|---|---|
| `pattern` | Regex | Match against output content |
| `message` | String | Replacement message (supports `$1`, `$2` capture groups) |
| `unless` | Regex | Skip this match if unless-pattern also matches |

## Testing Your Filters

```bash
# Verify all loaded filters pass inline tests
omni learn --verify

# Discovery: search for patterns in a log file
omni learn --status < output.log

# Preview: show generated TOML
omni learn --dry-run < output.log

# Action: apply patterns to learned.toml
omni learn --apply < output.log
```

## Project-local Filters (.omni/filters/)

This filter level is extremely useful for teams or repositories that have specific internal tooling. By placing filters inside the project folder, you ensure that the entire team (and their AI agents) gets consistent signal distillation.

### Usage Guide (Step-by-Step)

1. **Create Directory**: In your project root, create the `.omni/filters/` directory.
   ```bash
   mkdir -p .omni/filters
   ```

2. **Add Filter**: Create a TOML file, for example `.omni/filters/setup-backend.toml`.
   ```toml
   schema_version = 1
   [filters.setup-backend]
   match_command = "^./scripts/setup-db"
   strip_lines_matching = ["^Connecting", "^Checking versions"]
   on_empty = "db: setup complete"
   ```

3. **Verify & Trust**: Run `omni doctor`. You will see the status **[WARNING] NOT TRUSTED**. Run the following command to enable it:
   ```bash
   omni trust
   ```

### Benefits of Project Filters
- **Checked into Git**: These filters can be added to the repository (`git add .omni/filters`), ensuring anyone who clones the repo immediately gets the same token savings.
- **Custom Signalling**: Perfect for internal scripts that produce noisy output but only contain small bits of information relevant to the AI.

## Trust for Project Filters

Project-local filters (`.omni/filters/`) will not be loaded until you explicitly "trust" the project. This is a security feature to prevent malicious filters from untrusted repositories.

```bash
omni trust    # Review and approve filters in the current project
omni doctor   # Check trust status and number of loaded filters
```
