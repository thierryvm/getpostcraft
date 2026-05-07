#!/usr/bin/env node
/**
 * Verify that package.json / Cargo.toml / tauri.conf.json all advertise the same
 * version. With --strict, also enforce that the version matches the current git tag
 * (CI safety net so we never publish a release whose binary version mismatches its
 * download URL — that breaks the auto-updater).
 *
 * Exits non-zero on mismatch, zero on success. The `parseAll` export is used by the
 * unit tests so we don't have to spawn child processes there.
 */
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.join(__dirname, "..");

/** Pull the package.json version field. */
function parsePackageJson(content) {
  const pkg = JSON.parse(content);
  if (typeof pkg.version !== "string" || pkg.version.length === 0) {
    throw new Error("package.json: missing or non-string version");
  }
  return pkg.version;
}

/**
 * Pull the [package] version from a Cargo.toml. Naive parser — we only need the
 * top-level [package] section, and we want to avoid a TOML dep just for this.
 */
function parseCargoToml(content) {
  let inPackageSection = false;
  for (const rawLine of content.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (line.startsWith("#") || line.length === 0) continue;
    if (line.startsWith("[")) {
      inPackageSection = line === "[package]";
      continue;
    }
    if (!inPackageSection) continue;
    const match = line.match(/^version\s*=\s*"([^"]+)"/);
    if (match) return match[1];
  }
  throw new Error("Cargo.toml: no [package].version found");
}

/** Pull the .version from tauri.conf.json. */
function parseTauriConf(content) {
  const conf = JSON.parse(content);
  if (typeof conf.version !== "string" || conf.version.length === 0) {
    throw new Error("tauri.conf.json: missing or non-string version");
  }
  return conf.version;
}

/**
 * Read all three files and return the parsed versions. Exported for tests so they
 * can stub file contents without touching the real repo.
 */
export async function parseAll(read = readFile) {
  const [pkg, cargo, tauri] = await Promise.all([
    read(path.join(REPO_ROOT, "package.json"), "utf8"),
    read(path.join(REPO_ROOT, "src-tauri", "Cargo.toml"), "utf8"),
    read(path.join(REPO_ROOT, "src-tauri", "tauri.conf.json"), "utf8"),
  ]);
  return {
    packageJson: parsePackageJson(pkg),
    cargoToml: parseCargoToml(cargo),
    tauriConf: parseTauriConf(tauri),
  };
}

/**
 * Compare the three versions and return a list of human-readable error messages.
 * Empty array = all OK.
 */
export function compareVersions(versions, gitTag) {
  const errors = [];
  if (versions.packageJson !== versions.cargoToml) {
    errors.push(
      `package.json (${versions.packageJson}) ≠ Cargo.toml (${versions.cargoToml})`,
    );
  }
  if (versions.packageJson !== versions.tauriConf) {
    errors.push(
      `package.json (${versions.packageJson}) ≠ tauri.conf.json (${versions.tauriConf})`,
    );
  }
  if (gitTag) {
    // Tags are conventionally prefixed "v" — strip before comparing
    const tagVersion = gitTag.replace(/^v/, "");
    if (tagVersion !== versions.packageJson) {
      errors.push(
        `git tag (${gitTag} → ${tagVersion}) ≠ package.json (${versions.packageJson})`,
      );
    }
  }
  return errors;
}

/**
 * Resolve which git tag to verify against, given an env var bag.
 *
 * Precedence: `RELEASE_TAG` (explicit input from `workflow_dispatch`) wins
 * over `GITHUB_REF_NAME` (set by GitHub for any workflow trigger). On a
 * branch dispatch GITHUB_REF_NAME is "main" even when the operator passed
 * a v0.x.0 tag — using it would always fail. Empty / whitespace values
 * are treated as unset.
 *
 * Exported separately so tests can pin the precedence without spawning
 * the script in a child process.
 */
export function resolveGitTag(env) {
  const pick = (k) => {
    const v = env[k];
    return typeof v === "string" && v.trim() !== "" ? v.trim() : null;
  };
  return pick("RELEASE_TAG") ?? pick("GITHUB_REF_NAME") ?? null;
}

async function main() {
  const args = process.argv.slice(2);
  const strict = args.includes("--strict");
  const gitTag = strict ? resolveGitTag(process.env) : null;

  const versions = await parseAll();
  const errors = compareVersions(versions, gitTag);

  console.log("Versions:");
  console.log(`  package.json    : ${versions.packageJson}`);
  console.log(`  Cargo.toml      : ${versions.cargoToml}`);
  console.log(`  tauri.conf.json : ${versions.tauriConf}`);
  if (gitTag) console.log(`  git tag         : ${gitTag}`);

  if (errors.length > 0) {
    console.error("\nVersion mismatch detected:");
    for (const err of errors) console.error(`  - ${err}`);
    process.exit(1);
  }
  console.log("\nAll versions aligned.");
}

// Only run main() when invoked directly (not when imported by tests).
const isMain =
  process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1]);
if (isMain) {
  main().catch((e) => {
    console.error(`Fatal: ${e.message}`);
    process.exit(1);
  });
}
