#!/usr/bin/env node
/**
 * Build the auto-updater manifest (`latest.json`) from the assets attached to a
 * GitHub release. Each Tauri bundle target uploads a binary + a `.sig` file; the
 * manifest tells the running app which URL to download for its platform and the
 * Ed25519 signature to verify.
 *
 * Inputs (env):
 *   - RELEASE_TAG    : the tag whose assets to manifest (e.g. v0.2.0)
 *   - GITHUB_TOKEN   : token with read access to the release
 *   - GITHUB_REPO    : owner/repo, defaults to thierryvm/getpostcraft
 *
 * Output: writes `latest.json` next to the script invocation.
 *
 * Exported helpers (`buildManifest`, `pickBundleAsset`) are unit-tested without
 * hitting the GitHub API.
 */
import { writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const DEFAULT_REPO = "thierryvm/getpostcraft";

/**
 * Tauri bundle file extensions per platform key. Listed in the order the updater
 * resolves them — first match wins. The `.sig` siblings are always present.
 */
const BUNDLE_PATTERNS = {
  "darwin-aarch64": [/_aarch64\.app\.tar\.gz$/, /_universal\.app\.tar\.gz$/],
  "darwin-x86_64": [/_x64\.app\.tar\.gz$/, /_universal\.app\.tar\.gz$/],
  "linux-x86_64": [/_amd64\.AppImage$/, /\.AppImage$/],
  "windows-x86_64": [/_x64-setup\.exe$/, /_x64-setup\.nsis\.zip$/, /_x64\.msi$/],
};

/**
 * Pick the most appropriate bundle for a platform from a set of release assets.
 * Returns `{ url, sigUrl }` or null if no match.
 */
export function pickBundleAsset(platform, assets) {
  const patterns = BUNDLE_PATTERNS[platform];
  if (!patterns) return null;
  for (const pattern of patterns) {
    const bundle = assets.find((a) => pattern.test(a.name));
    if (!bundle) continue;
    const sig = assets.find((a) => a.name === `${bundle.name}.sig`);
    if (!sig) continue; // bundle without signature is unsafe to ship
    return {
      url: bundle.browser_download_url,
      sigUrl: sig.browser_download_url,
    };
  }
  return null;
}

/**
 * Build the manifest object from release metadata + per-platform signatures.
 * `signatures` is a map `{ "darwin-x86_64": "<sig content>", ... }`.
 */
export function buildManifest({ tag, notes, pubDate, assets, signatures }) {
  const version = tag.replace(/^v/, "");
  const platforms = {};
  for (const platform of Object.keys(BUNDLE_PATTERNS)) {
    const picked = pickBundleAsset(platform, assets);
    if (!picked) continue;
    const signature = signatures[platform];
    if (!signature) continue;
    platforms[platform] = {
      signature,
      url: picked.url,
    };
  }
  return {
    version,
    notes: notes ?? "",
    pub_date: pubDate ?? new Date().toISOString(),
    platforms,
  };
}

async function ghJson(token, urlPath, repo) {
  const res = await fetch(`https://api.github.com/repos/${repo}${urlPath}`, {
    headers: {
      Authorization: `Bearer ${token}`,
      Accept: "application/vnd.github+json",
      "X-GitHub-Api-Version": "2022-11-28",
    },
  });
  if (!res.ok) throw new Error(`GitHub API ${res.status} on ${urlPath}: ${await res.text()}`);
  return res.json();
}

/**
 * Find a release by tag, including drafts.
 *
 * `/releases/tags/{tag}` only returns published releases — drafts are 404. Our
 * release.yml workflow leaves the release as a draft after the matrix build so
 * the operator can review before publishing, so we MUST be able to find drafts
 * here. Solution: list all releases (the list endpoint includes drafts when
 * authenticated with `contents: write`) and filter by `tag_name`.
 *
 * Exported for unit testing — pure filter, no I/O.
 */
export function findReleaseByTag(releases, tag) {
  if (!Array.isArray(releases)) {
    throw new TypeError(`Expected array of releases, got ${typeof releases}`);
  }
  return releases.find((r) => r && r.tag_name === tag) ?? null;
}

async function ghDownloadText(token, url) {
  const res = await fetch(url, {
    headers: {
      Authorization: `Bearer ${token}`,
      Accept: "application/octet-stream",
    },
  });
  if (!res.ok) throw new Error(`Download ${res.status} on ${url}`);
  return res.text();
}

async function main() {
  const tag = process.env.RELEASE_TAG;
  const token = process.env.GITHUB_TOKEN;
  const repo = process.env.GITHUB_REPO ?? DEFAULT_REPO;
  if (!tag) throw new Error("RELEASE_TAG env var required");
  if (!token) throw new Error("GITHUB_TOKEN env var required");

  // List all releases including drafts. The /releases/tags/{tag} endpoint
  // returns 404 for drafts even with write scope, so we list-then-filter.
  // 100 releases is plenty: a project would publish that many over years.
  const releases = await ghJson(token, `/releases?per_page=100`, repo);
  const release = findReleaseByTag(releases, tag);
  if (!release) {
    throw new Error(
      `Tag ${tag} not found in any release (including drafts) on ${repo}`,
    );
  }
  const assets = release.assets ?? [];

  // Pull each .sig file content for the platforms we have a bundle for.
  const signatures = {};
  for (const platform of Object.keys(BUNDLE_PATTERNS)) {
    const picked = pickBundleAsset(platform, assets);
    if (!picked) continue;
    signatures[platform] = (await ghDownloadText(token, picked.sigUrl)).trim();
  }

  const manifest = buildManifest({
    tag,
    notes: release.body,
    pubDate: release.published_at,
    assets,
    signatures,
  });

  const outPath = path.resolve("latest.json");
  await writeFile(outPath, JSON.stringify(manifest, null, 2), "utf8");
  console.log(`Wrote ${outPath} for ${Object.keys(manifest.platforms).length} platform(s).`);
}

const isMain =
  process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1]);
if (isMain) {
  main().catch((e) => {
    console.error(`Fatal: ${e.message}`);
    process.exit(1);
  });
}
