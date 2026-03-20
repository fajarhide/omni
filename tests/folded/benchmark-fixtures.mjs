import fs from "fs";
import path from "path";
import { execFileSync } from "child_process";
import { fileURLToPath } from "url";

const FIXTURE_CASES = [
  ["npm-install", "tests/fixtures/npm_install_full.txt"],
  ["docker-build", "tests/fixtures/docker_build.txt"],
  ["git-diff", "tests/fixtures/git_diff.txt"],
  ["pytest-fail", "tests/fixtures/pytest_fail.txt"],
  ["tsc-errors", "tests/fixtures/tsc_errors.txt"],
  ["cat-plain", "tests/fixtures/cat_plain.txt"],
  ["jest-fail", "tests/fixtures/jest_fail.txt"],
  ["vite-build", "tests/fixtures/vite_build.txt"],
];

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..", "..");
const DEFAULT_BASELINE_PATH = path.resolve(__dirname, "benchmark-baseline.json");
const OMNI_BIN = fs.existsSync(path.resolve(REPO_ROOT, "core", "zig-out", "bin", "omni"))
  ? path.resolve(REPO_ROOT, "core", "zig-out", "bin", "omni")
  : "omni";

function parseArgs(argv) {
  const options = {
    json: false,
    strict: false,
    writeBaseline: false,
    baselinePath: DEFAULT_BASELINE_PATH,
    savingsTolerance: 0.05,
    avgTolerance: 0.03,
  };

  for (let i = 2; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--json") options.json = true;
    else if (arg === "--strict") options.strict = true;
    else if (arg === "--write-baseline") options.writeBaseline = true;
    else if (arg.startsWith("--baseline=")) options.baselinePath = path.resolve(arg.slice(11));
    else if (arg.startsWith("--savings-tolerance=")) options.savingsTolerance = Number(arg.slice(20));
    else if (arg.startsWith("--avg-tolerance=")) options.avgTolerance = Number(arg.slice(16));
  }

  return options;
}

function countLines(text) {
  if (text.length === 0) return 0;
  return text.endsWith("\n") ? text.split("\n").length - 1 : text.split("\n").length;
}

function shellQuote(value) {
  return `'${value.replace(/'/g, `'\\''`)}'`;
}

function runOmni(inputPath) {
  return execFileSync("sh", ["-lc", `cat ${shellQuote(inputPath)} | ${shellQuote(OMNI_BIN)}`], {
    encoding: "utf8",
  });
}

function summarizeQuality(raw, distilled) {
  const issues = [];
  if (/\{[a-z_][a-z0-9_]*\}/i.test(distilled)) issues.push("unresolved-placeholder");
  if (distilled.includes("[auto-filtered]")) issues.push("auto-filtered");
  if (distilled.includes("[OMNI Context Manifest:")) issues.push("manifest-wrapper");
  if (raw.length >= 250 && distilled.trim().length <= 40) issues.push("very-short-output");

  if (issues.length === 0) return "good";
  if (issues.includes("unresolved-placeholder") || issues.includes("auto-filtered")) return "aggressive";
  return "mixed";
}

function pct(savedFraction) {
  return `${(savedFraction * 100).toFixed(1)}%`;
}

function preview(text) {
  return text.replace(/\s+/g, " ").trim().slice(0, 110);
}

function qualityRank(quality) {
  return { good: 0, mixed: 1, aggressive: 2 }[quality] ?? 99;
}

function buildResults() {
  return FIXTURE_CASES.map(([name, relPath]) => {
    const absPath = path.resolve(REPO_ROOT, relPath);
    const raw = fs.readFileSync(absPath, "utf8");
    const distilled = runOmni(absPath);
    const savedFraction = raw.length === 0 ? 0 : 1 - distilled.length / raw.length;

    return {
      name,
      fixture_path: relPath,
      raw_chars: raw.length,
      raw_lines: countLines(raw),
      omni_chars: distilled.length,
      omni_lines: countLines(distilled),
      saved_fraction: Number(savedFraction.toFixed(6)),
      quality: summarizeQuality(raw, distilled),
      preview: preview(distilled),
    };
  });
}

function buildPayload(results) {
  const averageSavings =
    results.reduce((sum, item) => sum + item.saved_fraction, 0) / Math.max(results.length, 1);

  return {
    version: 1,
    generated_at: new Date().toISOString(),
    omni_bin: OMNI_BIN,
    average_saved_fraction: Number(averageSavings.toFixed(6)),
    cases: results,
  };
}

function ensureDir(filePath) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
}

function loadBaseline(baselinePath) {
  if (!fs.existsSync(baselinePath)) return null;
  return JSON.parse(fs.readFileSync(baselinePath, "utf8"));
}

function compareToBaseline(payload, baseline, options) {
  if (!baseline) return [];

  const failures = [];
  const baselineCases = new Map((baseline.cases ?? []).map((item) => [item.name, item]));

  for (const current of payload.cases) {
    const prior = baselineCases.get(current.name);
    if (!prior) continue;

    if (qualityRank(current.quality) > qualityRank(prior.quality)) {
      failures.push(
        `${current.name}: quality regressed from ${prior.quality} to ${current.quality}`,
      );
    }

    if (current.saved_fraction + options.savingsTolerance < prior.saved_fraction) {
      failures.push(
        `${current.name}: savings regressed from ${pct(prior.saved_fraction)} to ${pct(current.saved_fraction)}`,
      );
    }
  }

  if (
    typeof baseline.average_saved_fraction === "number" &&
    payload.average_saved_fraction + options.avgTolerance < baseline.average_saved_fraction
  ) {
    failures.push(
      `average savings regressed from ${pct(baseline.average_saved_fraction)} to ${pct(payload.average_saved_fraction)}`,
    );
  }

  return failures;
}

function printMarkdown(payload, baselineFailures) {
  console.log("# OMNI Fixture Benchmark");
  console.log("");
  console.log("| Case | Raw chars | OMNI chars | Raw lines | OMNI lines | Saved | Quality | Preview |");
  console.log("| --- | ---: | ---: | ---: | ---: | ---: | --- | --- |");
  for (const item of payload.cases) {
    console.log(
      `| ${item.name} | ${item.raw_chars} | ${item.omni_chars} | ${item.raw_lines} | ${item.omni_lines} | ${pct(item.saved_fraction)} | ${item.quality} | ${item.preview} |`,
    );
  }
  console.log("");
  console.log(`Average savings: ${pct(payload.average_saved_fraction)}`);

  const flagged = payload.cases.filter((item) => item.quality !== "good");
  if (flagged.length > 0) {
    console.log("");
    console.log("Flagged cases:");
    for (const item of flagged) {
      console.log(`- ${item.name}: ${item.quality}`);
    }
  }

  if (baselineFailures.length > 0) {
    console.log("");
    console.log("Regression failures:");
    for (const failure of baselineFailures) {
      console.log(`- ${failure}`);
    }
  }
}

const options = parseArgs(process.argv);
const results = buildResults();
const payload = buildPayload(results);

if (options.writeBaseline) {
  ensureDir(options.baselinePath);
  fs.writeFileSync(options.baselinePath, JSON.stringify(payload, null, 2) + "\n");
}

const baseline = loadBaseline(options.baselinePath);
const baselineFailures = compareToBaseline(payload, baseline, options);

if (options.json) {
  console.log(JSON.stringify({ ...payload, baseline_failures: baselineFailures }, null, 2));
} else {
  printMarkdown(payload, baselineFailures);
}

if (options.strict && baselineFailures.length > 0) {
  process.exitCode = 1;
}
