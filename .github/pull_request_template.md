## Checklist

- [ ] **Infrastructure & Integrity**:
    - [ ] Versions are consistent across all project files (`make check-version`).
    - [ ] Native Core (Zig) and Wasm build successfully (`make build-wasm`).
    - [ ] TypeScript MCP Server compiles successfully (`make build-ts`).
- [ ] **Functionality & Testing**:
    - [ ] All Zig core unit tests pass (`cd core && zig build test`).
    - [ ] Semantic routing verification suite passes (`make test`).
    - [ ] `omni report` branding and metrics generation verified (`make report`).
- [ ] **Security & Privacy**:
    - [ ] **Zero Telemetry**: Confirmed no external network calls or data exfiltration.
    - [ ] Data is stored locally and securely in `~/.omni/metrics.csv`.
- [ ] **Documentation**:
    - [ ] `CHANGELOG.md` updated with relevant version/change details.
    - [ ] Public documentation in `docs/` or `README.md` updated if necessary.
- [ ] **Verification**:
    - [ ] Full pre-release suite passed: `make verify`.

## Screenshots / CLI Output
<!-- Paste command output or screenshots demonstrating the change below -->
