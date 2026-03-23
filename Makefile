.PHONY: all fmt clippy test security binary-check ci clean

# The default target runs the full CI pipeline
all: ci

fmt:
	@echo "=== Running Rustfmt ==="
	cargo fmt --check

clippy:
	@echo "=== Running Clippy ==="
	cargo clippy --all-targets -- -D warnings

test:
	@echo "=== Running Tests ==="
	cargo test --all
	@echo "=== Verifying Snapshots ==="
	cargo insta test

security:
	@echo "=== Running Security Audit ==="
	cargo audit
	@echo "=== Checking for dangerous patterns ==="
	@FAIL=0; \
	if grep -rn 'Command::new("sh")' src/hooks/; then \
		echo "WARNING: Command::new(\"sh\") found in hooks"; \
		FAIL=1; \
	fi; \
	if grep -rn 'LD_PRELOAD' src/ --include='*.rs' | grep -v 'guard/env.rs' | grep -v 'DENYLIST'; then \
		echo "WARNING: LD_PRELOAD referenced outside guard/env.rs"; \
		FAIL=1; \
	fi; \
	UNWRAP_COUNT=$$(grep -rn '\.unwrap()' src/hooks/ --include='*.rs' | grep -v '#\[test\]' | grep -v 'mod tests' | grep -v '// safe:' | wc -l | tr -d ' '); \
	if [ "$$UNWRAP_COUNT" -gt 5 ]; then \
		echo "WARNING: $$UNWRAP_COUNT unwrap() calls in src/hooks/ (max 5 allowed)"; \
		grep -rn '\.unwrap()' src/hooks/ --include='*.rs' | grep -v '#\[test\]' | grep -v 'mod tests'; \
		FAIL=1; \
	fi; \
	if [ $$FAIL -eq 1 ]; then \
		echo "Security checks found issues. Review above."; \
		exit 1; \
	fi; \
	echo "All security pattern checks passed ✓"

binary-check:
	@echo "=== Building Release Binary ==="
	cargo build --release
	@echo "=== Checking Binary Size ==="
	@SIZE=$$(stat -c%s target/release/omni 2>/dev/null || stat -f%z target/release/omni); \
	SIZE_MB=$$((SIZE / 1048576)); \
	echo "Binary size: $${SIZE_MB}MB ($${SIZE} bytes)"; \
	if [ $$SIZE -gt 15728640 ]; then \
		echo "ERROR: Binary exceeds 15MB limit"; \
		exit 1; \
	fi; \
	echo "Binary size check passed ✓"
	@echo "=== Running Smoke Tests ==="
	chmod +x tests/smoke_test.sh
	tests/smoke_test.sh ./target/release/omni

ci: fmt clippy test security binary-check
	@echo "========================================"
	@echo "🚀 All CI checks passed successfully! 🚀"
	@echo "========================================"

clean:
	cargo clean

bump:
	@if [ -z "$(VERSION)" ]; then echo "Usage: make bump VERSION=0.5.1"; exit 1; fi
	./scripts/bump_version.sh $(VERSION)

release-sha:
	@if [ -z "$(VERSION)" ]; then echo "Usage: make release-sha VERSION=0.5.1"; exit 1; fi
	./scripts/update_homebrew_sha.sh $(VERSION)

release:
	@if [ -z "$(VERSION)" ]; then echo "Usage: make release VERSION=0.5.1"; exit 1; fi
	./scripts/omni-release.sh $(VERSION)
