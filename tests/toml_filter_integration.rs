//! Integration tests for TOML filter command matching and output filtering.
//!
//! These tests load the actual filter TOML files from `filters/` and verify
//! that each filter's `match_command` regex correctly matches all expected
//! command variants. Negative tests ensure no false-positive cross-matching.
//!
//! To add tests for a new filter, add a new module below following the pattern.

use std::path::Path;

// ─── Helpers ────────────────────────────────────────────────────────────

/// Load all filters from the project's filters/ directory.
fn load_filters() -> Vec<omni::pipeline::toml_filter::TomlFilter> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let filters_dir = Path::new(&manifest_dir).join("filters");
    omni::pipeline::toml_filter::load_from_dir(&filters_dir).filters
}

/// Find a filter by exact name.
fn get_filter<'a>(
    filters: &'a [omni::pipeline::toml_filter::TomlFilter],
    name: &str,
) -> &'a omni::pipeline::toml_filter::TomlFilter {
    filters
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("filter '{}' not found", name))
}

/// Assert that a filter matches a given command string.
fn assert_matches(filter: &omni::pipeline::toml_filter::TomlFilter, cmd: &str) {
    assert!(
        filter.matches(cmd),
        "filter '{}' should match command '{}'",
        filter.name,
        cmd
    );
}

/// Assert that a filter does NOT match a given command string.
fn assert_not_matches(filter: &omni::pipeline::toml_filter::TomlFilter, cmd: &str) {
    assert!(
        !filter.matches(cmd),
        "filter '{}' should NOT match command '{}'",
        filter.name,
        cmd
    );
}

// ═══════════════════════════════════════════════════════════════════════
//  JS / TS Ecosystem
// ═══════════════════════════════════════════════════════════════════════

mod npm {
    use super::*;

    #[test]
    fn matches_install_variants() {
        let filters = load_filters();
        let f = get_filter(&filters, "npm");
        assert_matches(f, "npm install");
        assert_matches(f, "npm ci");
        assert_matches(f, "yarn add lodash");
        assert_matches(f, "pnpm add -D typescript");
        assert_matches(f, "bun add react");
        assert_matches(f, "npm remove express");
    }

    #[test]
    fn matches_run_variants() {
        let filters = load_filters();
        let f = get_filter(&filters, "npm");
        assert_matches(f, "npm run build");
        assert_matches(f, "npm run test");
        assert_matches(f, "npm run dev");
        assert_matches(f, "npm run lint");
        assert_matches(f, "pnpm run dev");
        assert_matches(f, "yarn run lint");
        assert_matches(f, "bun run build");
    }

    #[test]
    fn does_not_match_unrelated() {
        let filters = load_filters();
        let f = get_filter(&filters, "npm");
        assert_not_matches(f, "node index.js");
        assert_not_matches(f, "npx create-react-app");
        assert_not_matches(f, "cargo build");
    }
}

mod npm_audit {
    use super::*;

    #[test]
    fn matches_npm_audit() {
        let filters = load_filters();
        let f = get_filter(&filters, "npm_audit");
        assert_matches(f, "npm audit");
        assert_matches(f, "npm audit --json");
    }

    #[test]
    fn does_not_match_unrelated() {
        let filters = load_filters();
        let f = get_filter(&filters, "npm_audit");
        assert_not_matches(f, "npm install");
        assert_not_matches(f, "yarn audit");
    }
}

mod vitest {
    use super::*;

    #[test]
    fn matches_vitest_and_jest() {
        let filters = load_filters();
        let f = get_filter(&filters, "vitest");
        assert_matches(f, "vitest");
        assert_matches(f, "vitest run");
        assert_matches(f, "jest");
        assert_matches(f, "jest --coverage");
        assert_matches(f, "npm test");
        assert_matches(f, "pnpm test");
    }

    #[test]
    fn matches_bun_test() {
        let filters = load_filters();
        let f = get_filter(&filters, "vitest");
        assert_matches(f, "bun test");
        assert_matches(f, "bun test src/");
        assert_matches(f, "bun run test");
    }

    #[test]
    fn does_not_match_unrelated() {
        let filters = load_filters();
        let f = get_filter(&filters, "vitest");
        assert_not_matches(f, "cargo test");
        assert_not_matches(f, "go test ./...");
        assert_not_matches(f, "pytest");
    }
}

mod eslint {
    use super::*;

    #[test]
    fn matches_eslint_biome_prettier() {
        let filters = load_filters();
        let f = get_filter(&filters, "eslint");
        assert_matches(f, "eslint .");
        assert_matches(f, "eslint src/ --fix");
        assert_matches(f, "biome check src/");
        assert_matches(f, "prettier --write .");
    }

    #[test]
    fn does_not_match_unrelated() {
        let filters = load_filters();
        let f = get_filter(&filters, "eslint");
        assert_not_matches(f, "npm run lint");
        assert_not_matches(f, "cargo clippy");
    }
}

mod tsc {
    use super::*;

    #[test]
    fn matches_tsc() {
        let filters = load_filters();
        let f = get_filter(&filters, "tsc");
        assert_matches(f, "tsc");
        assert_matches(f, "tsc --noEmit");
        assert_matches(f, "npx tsc --build");
    }

    #[test]
    fn does_not_match_unrelated() {
        let filters = load_filters();
        let f = get_filter(&filters, "tsc");
        assert_not_matches(f, "node index.ts");
        assert_not_matches(f, "cargo build");
    }
}

mod bundle {
    use super::*;

    #[test]
    fn matches_bundle() {
        let filters = load_filters();
        let f = get_filter(&filters, "bundle");
        assert_matches(f, "bundle install");
        assert_matches(f, "bundle update");
    }

    #[test]
    fn does_not_match_unrelated() {
        let filters = load_filters();
        let f = get_filter(&filters, "bundle");
        assert_not_matches(f, "bundle exec rake");
        assert_not_matches(f, "npm install");
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Python Ecosystem
// ═══════════════════════════════════════════════════════════════════════

mod pytest {
    use super::*;

    #[test]
    fn matches_bare_pytest() {
        let filters = load_filters();
        let f = get_filter(&filters, "pytest");
        assert_matches(f, "pytest");
        assert_matches(f, "pytest tests/ -v");
        assert_matches(f, "pytest --cov=src tests/");
    }

    #[test]
    fn matches_python_m_pytest() {
        let filters = load_filters();
        let f = get_filter(&filters, "pytest");
        assert_matches(f, "python -m pytest");
        assert_matches(f, "python3 -m pytest");
        assert_matches(f, "python -m pytest tests/ -v --cov");
        assert_matches(f, "python3 -m pytest -x --tb=short");
    }

    #[test]
    fn does_not_match_unrelated() {
        let filters = load_filters();
        let f = get_filter(&filters, "pytest");
        assert_not_matches(f, "python manage.py test");
        assert_not_matches(f, "python setup.py");
        assert_not_matches(f, "pip install pytest");
    }
}

mod pip {
    use super::*;

    #[test]
    fn matches_pip() {
        let filters = load_filters();
        let f = get_filter(&filters, "pip");
        assert_matches(f, "pip install flask");
        assert_matches(f, "pip install -r requirements.txt");
        assert_matches(f, "pip uninstall requests");
        assert_matches(f, "pip download numpy");
        assert_matches(f, "pip wheel .");
    }

    #[test]
    fn does_not_match_unrelated() {
        let filters = load_filters();
        let f = get_filter(&filters, "pip");
        assert_not_matches(f, "pip list");
        assert_not_matches(f, "pip freeze");
        assert_not_matches(f, "python -m pip");
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Rust / Go Ecosystem
// ═══════════════════════════════════════════════════════════════════════

mod cargo {
    use super::*;

    #[test]
    fn matches_cargo() {
        let filters = load_filters();
        let f = get_filter(&filters, "cargo");
        assert_matches(f, "cargo build");
        assert_matches(f, "cargo test");
        assert_matches(f, "cargo build --release");
        assert_matches(f, "cargo clippy");
        assert_matches(f, "rustc --edition 2021 main.rs");
    }

    #[test]
    fn does_not_match_unrelated() {
        let filters = load_filters();
        let f = get_filter(&filters, "cargo");
        assert_not_matches(f, "npm install");
        assert_not_matches(f, "go build");
    }
}

mod go_test {
    use super::*;

    #[test]
    fn matches_go() {
        let filters = load_filters();
        let f = get_filter(&filters, "go_test");
        assert_matches(f, "go test ./...");
        assert_matches(f, "go test -v -race ./pkg/...");
        assert_matches(f, "go build -o bin/app ./cmd/...");
        assert_matches(f, "go vet ./...");
    }

    #[test]
    fn does_not_match_unrelated() {
        let filters = load_filters();
        let f = get_filter(&filters, "go_test");
        assert_not_matches(f, "go run main.go");
        assert_not_matches(f, "cargo test");
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  PHP / Ruby Ecosystem
// ═══════════════════════════════════════════════════════════════════════

mod phpunit {
    use super::*;

    #[test]
    fn matches_phpunit() {
        let filters = load_filters();
        let f = get_filter(&filters, "phpunit");
        assert_matches(f, "phpunit");
        assert_matches(f, "phpunit --filter=testLogin");
        assert_matches(f, "php artisan test");
        assert_matches(f, "php artisan test --parallel");
    }

    #[test]
    fn does_not_match_unrelated() {
        let filters = load_filters();
        let f = get_filter(&filters, "phpunit");
        assert_not_matches(f, "php -S localhost:8000");
        assert_not_matches(f, "composer install");
    }
}

mod rspec {
    use super::*;

    #[test]
    fn matches_rspec() {
        let filters = load_filters();
        let f = get_filter(&filters, "rspec");
        assert_matches(f, "rspec");
        assert_matches(f, "rspec spec/models/user_spec.rb");
        assert_matches(f, "rspec --format documentation");
    }

    #[test]
    fn does_not_match_unrelated() {
        let filters = load_filters();
        let f = get_filter(&filters, "rspec");
        assert_not_matches(f, "bundle exec rake");
        assert_not_matches(f, "rails test");
    }
}

mod composer {
    use super::*;

    #[test]
    fn matches_composer() {
        let filters = load_filters();
        let f = get_filter(&filters, "composer");
        assert_matches(f, "composer install");
        assert_matches(f, "composer update");
        assert_matches(f, "composer require laravel/framework");
    }

    #[test]
    fn does_not_match_unrelated() {
        let filters = load_filters();
        let f = get_filter(&filters, "composer");
        assert_not_matches(f, "composer dump-autoload");
        assert_not_matches(f, "php artisan serve");
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  DevOps / Infrastructure
// ═══════════════════════════════════════════════════════════════════════

mod docker_compose {
    use super::*;

    #[test]
    fn matches_docker_compose() {
        let filters = load_filters();
        let f = get_filter(&filters, "docker_compose");
        assert_matches(f, "docker compose up");
        assert_matches(f, "docker compose build");
        assert_matches(f, "docker-compose up -d");
        assert_matches(f, "docker compose down");
    }

    #[test]
    fn does_not_match_unrelated() {
        let filters = load_filters();
        let f = get_filter(&filters, "docker_compose");
        assert_not_matches(f, "docker build .");
        assert_not_matches(f, "docker run nginx");
    }
}

mod kubectl {
    use super::*;

    #[test]
    fn matches_kubectl() {
        let filters = load_filters();
        let f = get_filter(&filters, "kubectl");
        assert_matches(f, "kubectl get pods");
        assert_matches(f, "kubectl describe svc my-service");
        assert_matches(f, "kubectl apply -f deploy.yaml");
    }

    #[test]
    fn does_not_match_unrelated() {
        let filters = load_filters();
        let f = get_filter(&filters, "kubectl");
        assert_not_matches(f, "helm install my-release");
        assert_not_matches(f, "docker ps");
    }
}

mod kubectl_logs {
    use super::*;

    #[test]
    fn matches_kubectl_logs() {
        let filters = load_filters();
        let f = get_filter(&filters, "kubectl_logs");
        assert_matches(f, "kubectl logs my-pod");
        assert_matches(f, "kubectl logs -f deployment/my-app");
        assert_matches(f, "kubectl logs my-pod --tail=100");
    }

    #[test]
    fn does_not_match_unrelated() {
        let filters = load_filters();
        let f = get_filter(&filters, "kubectl_logs");
        assert_not_matches(f, "kubectl get pods");
        assert_not_matches(f, "docker logs container");
    }
}

mod terraform {
    use super::*;

    #[test]
    fn matches_terraform_plan() {
        let filters = load_filters();
        let f = get_filter(&filters, "terraform");
        assert_matches(f, "terraform plan");
        assert_matches(f, "terraform plan -out=tfplan");
    }

    #[test]
    fn matches_terraform_apply_destroy_init() {
        let filters = load_filters();
        let f = get_filter(&filters, "terraform");
        assert_matches(f, "terraform apply");
        assert_matches(f, "terraform apply -auto-approve");
        assert_matches(f, "terraform destroy");
        assert_matches(f, "terraform destroy -auto-approve");
        assert_matches(f, "terraform init");
        assert_matches(f, "terraform init -upgrade");
    }

    #[test]
    fn does_not_match_unrelated() {
        let filters = load_filters();
        let f = get_filter(&filters, "terraform");
        assert_not_matches(f, "terraform fmt");
        assert_not_matches(f, "terraform validate");
        assert_not_matches(f, "terrafirma plan");
    }
}

mod ansible {
    use super::*;

    #[test]
    fn matches_ansible() {
        let filters = load_filters();
        let f = get_filter(&filters, "ansible");
        assert_matches(f, "ansible-playbook site.yml");
        assert_matches(f, "ansible-playbook -i inventory deploy.yml");
    }

    #[test]
    fn does_not_match_unrelated() {
        let filters = load_filters();
        let f = get_filter(&filters, "ansible");
        assert_not_matches(f, "ansible-galaxy install role");
        assert_not_matches(f, "ansible --version");
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Git / Build Tools
// ═══════════════════════════════════════════════════════════════════════

mod git_log {
    use super::*;

    #[test]
    fn matches_git_log() {
        let filters = load_filters();
        let f = get_filter(&filters, "git_log");
        assert_matches(f, "git log");
        assert_matches(f, "git log --oneline -10");
        assert_matches(f, "git log --graph --all");
    }

    #[test]
    fn does_not_match_unrelated() {
        let filters = load_filters();
        let f = get_filter(&filters, "git_log");
        assert_not_matches(f, "git status");
        assert_not_matches(f, "git diff");
        assert_not_matches(f, "git commit -m 'msg'");
    }
}

mod make {
    use super::*;

    #[test]
    fn matches_make() {
        let filters = load_filters();
        let f = get_filter(&filters, "make");
        assert_matches(f, "make");
        assert_matches(f, "make build");
        assert_matches(f, "make -j4 all");
    }

    #[test]
    fn does_not_match_unrelated() {
        let filters = load_filters();
        let f = get_filter(&filters, "make");
        assert_not_matches(f, "cargo build");
        assert_not_matches(f, "npm run build");
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Cross-cutting: all inline tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn all_builtin_inline_tests_pass() {
    let filters = load_filters();
    let report = omni::pipeline::toml_filter::run_inline_tests(&filters);

    if !report.failures.is_empty() {
        for failure in &report.failures {
            eprintln!("{}", failure);
        }
        panic!(
            "{} inline TOML filter test(s) failed",
            report.failures.len()
        );
    }

    assert!(
        report.passes >= 10,
        "Expected at least 10 inline tests, got {}",
        report.passes
    );
}

/// Ensure every filter TOML file produced at least one loadable filter.
#[test]
fn all_filter_files_loaded() {
    let filters = load_filters();
    let expected = [
        "npm",
        "npm_audit",
        "vitest",
        "eslint",
        "tsc",
        "bundle",
        "pytest",
        "pip",
        "cargo",
        "go_test",
        "phpunit",
        "rspec",
        "composer",
        "docker_compose",
        "kubectl",
        "kubectl_logs",
        "terraform",
        "ansible",
        "git_log",
        "make",
    ];
    for name in expected {
        assert!(
            filters.iter().any(|f| f.name == name),
            "filter '{}' should be loaded from filters/ directory",
            name
        );
    }
}
