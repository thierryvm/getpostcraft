# Contributing to Getpostcraft

This is a solo project currently in active V1 development. This document describes the process and conventions used so that the codebase stays clean and the git history is readable.

---

## Branches

| Branch | Purpose |
|--------|---------|
| `main` | Stable, tagged releases only — protected |
| `develop` | Integration branch (merge target for features) |
| `feature/<slug>` | New feature or improvement |
| `fix/<slug>` | Bug fix |
| `hotfix/<slug>` | Critical fix directly off `main` |
| `release/vX.Y.Z` | Release preparation (changelog, version bump) |
| `chore/<slug>` | Tooling, CI, dependencies, docs |

## Commit messages

Format: `type(scope): short description`

| Type | When to use |
|------|------------|
| `feat` | New user-visible feature |
| `fix` | Bug fix |
| `refactor` | Code change with no behaviour change |
| `security` | Security improvement |
| `perf` | Performance improvement |
| `test` | Tests only |
| `docs` | Documentation only |
| `chore` | Tooling, CI, dependencies |

Rules:
- Imperative mood: "add X", not "added X"
- Max 72 characters in the subject line
- Body explains *why*, not *what* (the diff shows the what)
- Reference issues: `Closes #123` or `Refs #123`

Example:
```
feat(composer): add hashtag editing in ContentPreview

Users can now remove individual hashtags with the × button or add new
ones inline. Local state syncs whenever AI regenerates.

Closes #12
```

## Pull requests

Every PR must use the PR template (`.github/pull_request_template.md`).

Before opening a PR:
```bash
npm run typecheck          # TypeScript — 0 errors
cd src-tauri
cargo check                # Rust compile check
cargo clippy -- -D warnings  # Lint — 0 warnings
cargo fmt --check          # Format check
```

For UI changes, include before/after screenshots in the PR description.

## Issues

Use the issue templates:
- **Bug report** — something is broken
- **Feature request** — new capability
- **Chore / Tech debt** — refactor, dependency, CI

Label every issue before closing a PR against it.

## Versioning

[Semantic Versioning](https://semver.org): `MAJOR.MINOR.PATCH`

- `MAJOR` — breaking change in stored data format or IPC contract
- `MINOR` — new user-visible feature
- `PATCH` — bug fix, performance, security patch

Version is set in:
- `src-tauri/Cargo.toml` → `version`
- `src-tauri/tauri.conf.json` → `version`
- `package.json` → `version`
- `CHANGELOG.md` entry

## Changelog

Every PR of type `feat`, `fix`, or `security` must add an entry to `CHANGELOG.md` under `## [Unreleased]` following the [Keep a Changelog](https://keepachangelog.com) format.

## Architecture decisions

Significant technical decisions are documented as ADRs in `docs/adr/`. If a PR introduces a new architectural pattern or reverses a prior decision, open a new ADR or update the existing one.

## Security

- No secrets in code, commits, or logs — ever
- AI API keys stay in `%APPDATA%\getpostcraft\api_keys.json`, never cross IPC
- OAuth tokens never sent to the Python sidecar
- Run `npm audit` and `cargo audit` before release cuts

## Project structure

See `CLAUDE.md` for the full project map and coding conventions.
