import { describe, it, expect } from "vitest";
import { compareVersions, parseAll, resolveGitTag } from "./check-versions.mjs";

describe("compareVersions", () => {
  it("returns no errors when all three versions match", () => {
    const errors = compareVersions(
      { packageJson: "0.1.0", cargoToml: "0.1.0", tauriConf: "0.1.0" },
      null,
    );
    expect(errors).toEqual([]);
  });

  it("flags package.json vs Cargo.toml mismatch", () => {
    const errors = compareVersions(
      { packageJson: "0.1.0", cargoToml: "0.2.0", tauriConf: "0.1.0" },
      null,
    );
    expect(errors).toHaveLength(1);
    expect(errors[0]).toMatch(/package\.json.*Cargo\.toml/);
  });

  it("flags package.json vs tauri.conf.json mismatch", () => {
    const errors = compareVersions(
      { packageJson: "0.1.0", cargoToml: "0.1.0", tauriConf: "0.2.0" },
      null,
    );
    expect(errors).toHaveLength(1);
    expect(errors[0]).toMatch(/tauri\.conf\.json/);
  });

  it("flags both mismatches when all three differ", () => {
    const errors = compareVersions(
      { packageJson: "0.1.0", cargoToml: "0.2.0", tauriConf: "0.3.0" },
      null,
    );
    expect(errors).toHaveLength(2);
  });

  it("strips a leading v from the git tag before comparing", () => {
    const errors = compareVersions(
      { packageJson: "0.5.0", cargoToml: "0.5.0", tauriConf: "0.5.0" },
      "v0.5.0",
    );
    expect(errors).toEqual([]);
  });

  it("flags git tag mismatch with sources of truth", () => {
    const errors = compareVersions(
      { packageJson: "0.5.0", cargoToml: "0.5.0", tauriConf: "0.5.0" },
      "v0.6.0",
    );
    expect(errors).toHaveLength(1);
    expect(errors[0]).toMatch(/git tag/);
  });

  it("does not require a git tag when one is not provided", () => {
    const errors = compareVersions(
      { packageJson: "1.0.0", cargoToml: "1.0.0", tauriConf: "1.0.0" },
      null,
    );
    expect(errors).toEqual([]);
  });
});

describe("parseAll", () => {
  it("extracts version from each fixture file", async () => {
    const fakeFiles = {
      "package.json": JSON.stringify({ name: "x", version: "0.7.2" }),
      "Cargo.toml":
        '[package]\nname = "getpostcraft"\nversion = "0.7.2"\nedition = "2021"\n',
      "tauri.conf.json": JSON.stringify({ version: "0.7.2", productName: "x" }),
    };
    const fakeRead = async (filePath) => {
      const name = filePath.split(/[/\\]/).pop();
      if (!(name in fakeFiles)) throw new Error(`unexpected read: ${filePath}`);
      return fakeFiles[name];
    };
    const versions = await parseAll(fakeRead);
    expect(versions).toEqual({
      packageJson: "0.7.2",
      cargoToml: "0.7.2",
      tauriConf: "0.7.2",
    });
  });

  it("rejects a Cargo.toml without a [package].version", async () => {
    const fakeRead = async (filePath) => {
      const name = filePath.split(/[/\\]/).pop();
      if (name === "package.json") return JSON.stringify({ version: "1.0.0" });
      if (name === "Cargo.toml") return '[dependencies]\nfoo = "1"\n';
      if (name === "tauri.conf.json") return JSON.stringify({ version: "1.0.0" });
      throw new Error("unexpected");
    };
    await expect(parseAll(fakeRead)).rejects.toThrow(/no \[package\]\.version/);
  });

  it("ignores a 'version' line outside the [package] section", async () => {
    // Cargo.toml may carry a `version` key in `[dependencies]` like `serde = { version = "1" }`
    // — the parser must not pick that up.
    const cargoToml = [
      '[dependencies]',
      'serde = { version = "1.0", features = ["derive"] }',
      '',
      '[package]',
      'name = "getpostcraft"',
      'version = "0.9.9"',
    ].join("\n");
    const fakeRead = async (filePath) => {
      const name = filePath.split(/[/\\]/).pop();
      if (name === "package.json") return JSON.stringify({ version: "0.9.9" });
      if (name === "Cargo.toml") return cargoToml;
      if (name === "tauri.conf.json") return JSON.stringify({ version: "0.9.9" });
      throw new Error("unexpected");
    };
    const versions = await parseAll(fakeRead);
    expect(versions.cargoToml).toBe("0.9.9");
  });
});

describe("resolveGitTag", () => {
  it("prefers RELEASE_TAG over GITHUB_REF_NAME (regression — dispatch case)", () => {
    // Real-world failure that bricked the v0.2.0 workflow_dispatch re-run :
    // GITHUB_REF_NAME was "main" because we dispatched from main, but the
    // operator passed `tag=v0.2.0` as input → RELEASE_TAG. The old code
    // preferred GITHUB_REF_NAME and failed `git tag (main) ≠ 0.2.0`.
    const env = { RELEASE_TAG: "v0.2.0", GITHUB_REF_NAME: "main" };
    expect(resolveGitTag(env)).toBe("v0.2.0");
  });

  it("falls back to GITHUB_REF_NAME when RELEASE_TAG is unset (push tag case)", () => {
    const env = { GITHUB_REF_NAME: "v0.1.0" };
    expect(resolveGitTag(env)).toBe("v0.1.0");
  });

  it("returns null when both env vars are unset", () => {
    expect(resolveGitTag({})).toBeNull();
  });

  it("treats empty strings as unset", () => {
    expect(resolveGitTag({ RELEASE_TAG: "", GITHUB_REF_NAME: "" })).toBeNull();
    expect(
      resolveGitTag({ RELEASE_TAG: "", GITHUB_REF_NAME: "v0.1.0" }),
    ).toBe("v0.1.0");
  });

  it("treats whitespace-only values as unset", () => {
    expect(
      resolveGitTag({ RELEASE_TAG: "   ", GITHUB_REF_NAME: "v0.1.0" }),
    ).toBe("v0.1.0");
  });

  it("trims surrounding whitespace from the picked value", () => {
    // Some CI shells leak trailing newlines through env vars.
    expect(resolveGitTag({ RELEASE_TAG: " v0.2.0\n" })).toBe("v0.2.0");
  });
});
