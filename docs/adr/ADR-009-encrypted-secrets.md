# ADR-009 — OS-native encrypted secret storage

**Status:** ✅ Accepted (2026-05-08)
**Supersedes:** plain-text `api_keys.json` documented inline in `src/ai_keys.rs` (v0.1.0–v0.2.0)

## Context

Up to v0.2.0, every API key (OpenRouter, Anthropic, Instagram client_secret,
LinkedIn client_secret, imgbb) was stored in **plain text** at:

| OS      | Path                                                      |
|---------|-----------------------------------------------------------|
| Windows | `%APPDATA%\getpostcraft\api_keys.json`                    |
| macOS   | `~/Library/Application Support/getpostcraft/api_keys.json`|
| Linux   | `~/.local/share/getpostcraft/api_keys.json`               |

The legacy comment claimed *"file lives in the user's private data directory
(OS-level user isolation). Not encrypted, but not accessible to other users."*

That's only true for **other unprivileged users on the same machine**. It does
not protect against:
- A second admin account on a shared Windows machine.
- Malware running under the user's session (no sandbox boundary).
- Disk imaging during repair / theft.
- Cloud-sync / backup tools that copy `%APPDATA%` (OneDrive, syncthing).
- A future SaaS deployment where this app talks to a server.

For solo alpha use this is "fine"; for SaaS alpha this is **disqualifying**
(audit, GDPR, user trust). PR-A blocks every other security-sensitive feature
until this is resolved.

## Decision

Use the [`keyring`](https://crates.io/crates/keyring) crate (v3.x) which
abstracts the three OS-native secret stores:

| OS      | Backend                       | Encryption                          |
|---------|-------------------------------|-------------------------------------|
| Windows | Credential Manager (wincred)  | DPAPI bound to the user's Windows account |
| macOS   | Keychain Services              | User login keychain                 |
| Linux   | Secret Service over D-Bus      | Per-user keyring (gnome-keyring / KWallet) |

All three encrypt at rest with the OS user's credentials. Reading the secret
from another local user account, from a backup image, or from cloud sync now
yields an opaque blob the attacker cannot decrypt without the original user's
session token.

`keyring` adds ~250 KB to the binary; acceptable for the security gain.

## Considered alternatives

| Option | Why not |
|--------|---------|
| Native DPAPI directly via `windows` crate (Win-only) | We need cross-platform; would require 3× the code and 3× the test surface |
| Encrypt the JSON ourselves with a key derived from machine ID | Anyone with FS read access can derive the same key (machine ID is readable). Pure security theater |
| Stay plain-text | Disqualifying for SaaS, plus easy to fix |
| `secrecy` crate (in-memory zeroization) | Solves a different problem (RAM scraping); does not protect data at rest |

## Implementation

- `src-tauri/src/ai_keys.rs` rewritten on top of `keyring::Entry`.
- Service name `app.getpostcraft.secrets` namespaces our entries; provider name
  becomes the account.
- **One-time migration** at first call after upgrade: detect the legacy
  `api_keys.json`, copy each key into the keyring, delete the file. Idempotent
  (no-op once the file is gone). Failure is non-fatal: the file is preserved
  if any key fails to migrate so we can retry on the next run.
- `KNOWN_PROVIDERS` constant lists the secret names so `load_all()` can warm
  the in-memory cache (the keyring API does not expose enumeration).
- 7 unit tests run against the real OS keyring (`try_setup` skips gracefully
  on headless Linux without a session bus).

## Consequences

**Positive**
- Secrets at rest are now encrypted with the OS user's credentials on all 3
  supported desktops.
- Zero impact on the front-end / Tauri command surface — same function names
  and behavior. Migration is invisible.
- Unblocks PR-B (account_id wiring) and the SaaS roadmap.

**Negative / trade-offs**
- Tests touching the keyring **mutate process-shared state** — they must be
  serialized. Already implemented via `KEYRING_LOCK: Mutex<()>`.
- Headless Linux CI runners may not have a D-Bus session — tests use
  `try_setup` to skip gracefully. Production Linux desktops always have one.
- Adding a new secret type now requires editing `KNOWN_PROVIDERS`. A single
  unit test (`known_providers_list_covers_documented_secrets`) guards this.

## References

- [keyring crate](https://docs.rs/keyring/3)
- [Windows DPAPI](https://learn.microsoft.com/en-us/windows/win32/seccng/data-protection-api)
- [macOS Keychain Services](https://developer.apple.com/documentation/security/keychain_services)
- Issue origin: senior-dev Socratic analysis pendant la session du 2026-05-07
