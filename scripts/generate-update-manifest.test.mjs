import { describe, it, expect } from "vitest";
import {
  buildManifest,
  pickBundleAsset,
  findReleaseByTag,
} from "./generate-update-manifest.mjs";

/** Helper to build a fake GitHub release asset object. */
function asset(name) {
  return {
    name,
    browser_download_url: `https://github.com/owner/repo/releases/download/v1.0.0/${name}`,
  };
}

describe("pickBundleAsset", () => {
  it("matches the windows NSIS exe over the MSI", () => {
    const assets = [
      asset("getpostcraft_0.1.0_x64-setup.exe"),
      asset("getpostcraft_0.1.0_x64-setup.exe.sig"),
      asset("getpostcraft_0.1.0_x64.msi"),
      asset("getpostcraft_0.1.0_x64.msi.sig"),
    ];
    const picked = pickBundleAsset("windows-x86_64", assets);
    expect(picked?.url).toMatch(/_x64-setup\.exe$/);
    expect(picked?.sigUrl).toMatch(/_x64-setup\.exe\.sig$/);
  });

  it("falls back to MSI when no NSIS bundle is uploaded", () => {
    const assets = [
      asset("getpostcraft_0.1.0_x64.msi"),
      asset("getpostcraft_0.1.0_x64.msi.sig"),
    ];
    const picked = pickBundleAsset("windows-x86_64", assets);
    expect(picked?.url).toMatch(/_x64\.msi$/);
  });

  it("returns null when the bundle has no .sig sibling", () => {
    const assets = [asset("getpostcraft_0.1.0_x64-setup.exe")];
    const picked = pickBundleAsset("windows-x86_64", assets);
    expect(picked).toBeNull();
  });

  it("matches the macOS aarch64-specific bundle when available", () => {
    const assets = [
      asset("getpostcraft_aarch64.app.tar.gz"),
      asset("getpostcraft_aarch64.app.tar.gz.sig"),
      asset("getpostcraft_universal.app.tar.gz"),
      asset("getpostcraft_universal.app.tar.gz.sig"),
    ];
    const picked = pickBundleAsset("darwin-aarch64", assets);
    expect(picked?.url).toMatch(/_aarch64\.app\.tar\.gz$/);
  });

  it("falls back to the universal mac bundle when a per-arch one is missing", () => {
    const assets = [
      asset("getpostcraft_universal.app.tar.gz"),
      asset("getpostcraft_universal.app.tar.gz.sig"),
    ];
    const picked = pickBundleAsset("darwin-x86_64", assets);
    expect(picked?.url).toMatch(/_universal\.app\.tar\.gz$/);
  });

  it("matches a Linux AppImage", () => {
    const assets = [
      asset("getpostcraft_0.1.0_amd64.AppImage"),
      asset("getpostcraft_0.1.0_amd64.AppImage.sig"),
    ];
    const picked = pickBundleAsset("linux-x86_64", assets);
    expect(picked?.url).toMatch(/\.AppImage$/);
  });

  it("returns null for unknown platform keys", () => {
    expect(pickBundleAsset("freebsd", [])).toBeNull();
  });
});

describe("buildManifest", () => {
  const baseAssets = [
    asset("getpostcraft_0.2.0_x64-setup.exe"),
    asset("getpostcraft_0.2.0_x64-setup.exe.sig"),
    asset("getpostcraft_0.2.0_amd64.AppImage"),
    asset("getpostcraft_0.2.0_amd64.AppImage.sig"),
  ];

  it("strips a leading v from the tag and emits a SemVer string", () => {
    const manifest = buildManifest({
      tag: "v0.2.0",
      notes: "First public alpha",
      pubDate: "2026-05-07T15:00:00Z",
      assets: baseAssets,
      signatures: { "windows-x86_64": "SIG_W", "linux-x86_64": "SIG_L" },
    });
    expect(manifest.version).toBe("0.2.0");
  });

  it("only emits platforms with both a bundle and a signature", () => {
    const manifest = buildManifest({
      tag: "v0.2.0",
      notes: "",
      pubDate: "2026-05-07T15:00:00Z",
      assets: baseAssets,
      // macOS bundles aren't in baseAssets — manifest should skip those keys.
      signatures: {
        "windows-x86_64": "SIG_W",
        "linux-x86_64": "SIG_L",
        "darwin-x86_64": "SIG_M_THAT_HAS_NO_BUNDLE",
      },
    });
    expect(Object.keys(manifest.platforms).sort()).toEqual([
      "linux-x86_64",
      "windows-x86_64",
    ]);
  });

  it("drops a platform when its signature is missing even if a bundle exists", () => {
    const manifest = buildManifest({
      tag: "v0.2.0",
      notes: "",
      pubDate: "2026-05-07T15:00:00Z",
      assets: baseAssets,
      signatures: { "windows-x86_64": "SIG_W" }, // missing linux signature
    });
    expect(Object.keys(manifest.platforms)).toEqual(["windows-x86_64"]);
  });

  it("populates each platform entry with signature + url", () => {
    const manifest = buildManifest({
      tag: "v0.2.0",
      notes: "Notes",
      pubDate: "2026-05-07T15:00:00Z",
      assets: baseAssets,
      signatures: { "windows-x86_64": "SIG_W", "linux-x86_64": "SIG_L" },
    });
    expect(manifest.platforms["windows-x86_64"].signature).toBe("SIG_W");
    expect(manifest.platforms["windows-x86_64"].url).toMatch(
      /_x64-setup\.exe$/,
    );
    expect(manifest.platforms["linux-x86_64"].signature).toBe("SIG_L");
    expect(manifest.platforms["linux-x86_64"].url).toMatch(/\.AppImage$/);
  });

  it("defaults notes to empty string when not provided", () => {
    const manifest = buildManifest({
      tag: "v0.2.0",
      pubDate: "2026-05-07T15:00:00Z",
      assets: [],
      signatures: {},
    });
    expect(manifest.notes).toBe("");
  });

  it("defaults pub_date to the current ISO timestamp when not provided", () => {
    const manifest = buildManifest({
      tag: "v0.2.0",
      notes: "",
      assets: [],
      signatures: {},
    });
    expect(manifest.pub_date).toMatch(/^\d{4}-\d{2}-\d{2}T/);
  });
});

describe("findReleaseByTag", () => {
  /**
   * Regression: the v0.2.0 workflow failed with `404 on /releases/tags/v0.2.0`
   * because the release was still a draft. The workflow leaves the release as
   * draft so the operator can review before promoting — manifest publishing
   * MUST find drafts. These tests pin that behavior.
   */
  it("finds a draft release by tag (regression)", () => {
    const releases = [
      { tag_name: "v0.1.0", draft: false },
      { tag_name: "v0.2.0", draft: true }, // ← the case that failed in CI
      { tag_name: "v0.3.0-alpha", draft: true },
    ];
    expect(findReleaseByTag(releases, "v0.2.0")).toEqual({
      tag_name: "v0.2.0",
      draft: true,
    });
  });

  it("finds a published release by tag", () => {
    const releases = [
      { tag_name: "v0.1.0", draft: false },
      { tag_name: "v0.2.0", draft: false },
    ];
    expect(findReleaseByTag(releases, "v0.1.0")).toEqual({
      tag_name: "v0.1.0",
      draft: false,
    });
  });

  it("returns null when no matching tag exists", () => {
    const releases = [{ tag_name: "v0.1.0" }];
    expect(findReleaseByTag(releases, "v0.99.0")).toBeNull();
  });

  it("returns null on empty list", () => {
    expect(findReleaseByTag([], "v0.2.0")).toBeNull();
  });

  it("returns the first match if duplicates exist (defensive)", () => {
    // Should never happen on GitHub but guard against weird API responses.
    const releases = [
      { tag_name: "v0.2.0", draft: true, id: 1 },
      { tag_name: "v0.2.0", draft: false, id: 2 },
    ];
    expect(findReleaseByTag(releases, "v0.2.0").id).toBe(1);
  });

  it("ignores null entries in the array (defensive)", () => {
    const releases = [null, { tag_name: "v0.2.0", draft: true }, null];
    expect(findReleaseByTag(releases, "v0.2.0")).toEqual({
      tag_name: "v0.2.0",
      draft: true,
    });
  });

  it("throws on non-array input", () => {
    expect(() => findReleaseByTag("not an array", "v0.2.0")).toThrow(TypeError);
    expect(() => findReleaseByTag(null, "v0.2.0")).toThrow(TypeError);
    expect(() => findReleaseByTag({ releases: [] }, "v0.2.0")).toThrow(TypeError);
  });
});
