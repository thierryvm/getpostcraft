# Recovery — when the app won't start or your DB looks wrong

This guide covers the failure modes Getpostcraft is built to recover
from, and the order to try them in.

---

## Where your data lives

| What | Where | Survives uninstall? |
|---|---|---|
| App database (drafts, accounts, settings, post history) | `%APPDATA%\getpostcraft\app.db` (Windows) · `~/Library/Application Support/getpostcraft/app.db` (macOS) · `~/.local/share/getpostcraft/app.db` (Linux) | ✅ — installer never touches the data dir |
| API keys + OAuth tokens | OS keychain (DPAPI / Keychain / libsecret) | ✅ — keychain entries are never deleted by Getpostcraft |
| Daily auto-backup (`.gpcbak` ZIPs) | `~/Documents/Getpostcraft/backups/` | ✅ — Documents folder is outside the app dir |
| Pre-migration snapshot | `<data dir>/app.db.pre-migrate.bak` | ✅ — sits next to `app.db` |
| Rendered images cache | `<data dir>/renders/` | ✅ but disposable — deleted after upload |

Reinstalling never deletes any of the above. **Do not** run a "clean
reinstall" from Add/Remove Programs and then a fresh install if you
want to preserve data — just install the new version on top of the
old one.

---

## Symptom: the app closes immediately on launch

This usually means a panic during startup. Check the log first:

```
%LOCALAPPDATA%\app.getpostcraft\logs\app.log
```

The last `[ERROR]` line tells you what failed.

### "migration N was previously applied but has been modified"

Fixed automatically since v0.3.6 — `init_pool` heals stale checksums
in `_sqlx_migrations` before running migrations. If you see this on
v0.3.6 or later, please file an issue with the log.

If you're stuck on an older version that crashes with this:

1. Backup `app.db` first (just in case): copy it next to itself as
   `app.db.before-recovery.bak`.
2. Use any SQLite tool (DB Browser for SQLite, sqlite3 CLI) to run:
   ```sql
   -- Replace N with the migration number from the error.
   DELETE FROM _sqlx_migrations WHERE version = N;
   ```
3. Restart the app. sqlx will re-record the checksum on the next run.
   This is safe because the migration's body is idempotent
   (`CREATE TABLE IF NOT EXISTS`, `INSERT OR IGNORE`, …).
4. Upgrade to v0.3.6+ as soon as practical so the fix lands.

### "Failed to init SQLite: …" (other errors)

The schema may be corrupt. Roll back:

1. **First**, check whether `app.db.pre-migrate.bak` exists next to
   `app.db`. If it does AND you have NOT relaunched the app multiple
   times since the failure, this is your fastest rollback:
   ```powershell
   # Windows
   cd $env:APPDATA\getpostcraft
   Copy-Item app.db app.db.broken
   Copy-Item app.db.pre-migrate.bak app.db
   ```
   Then start the app.

   ⚠️ **Caveat**: the snapshot is overwritten on every successful
   startup. If you launched the app 2+ times after the migration
   failure, the snapshot may already reflect the broken state.
   Compare `app.db` and `app.db.pre-migrate.bak` byte sizes — if
   they're identical, fall through to the daily backup below.

2. **If the snapshot is gone or already broken**, restore from the
   daily auto-backup:
   - Open `~/Documents/Getpostcraft/backups/`
   - Pick the most recent `*.gpcbak` ZIP from before the failure
   - In Settings → Données → "Restaurer une sauvegarde", point at
     it. Restore is transactional — the live DB is replaced atomically.

3. **If neither is available** (very old install, never opened
   Documents): the keychain still holds your API keys + OAuth tokens.
   You'll lose drafts and post history, but reconnecting accounts in
   Settings → Comptes restores the publishing chain.

---

## Symptom: an upgrade made my carousels look weird

The Niveau A/B visual templates introduced in v0.3.6 changed colors,
layout, and badge mapping. If you preferred the prior look, the
fastest path forward is to roll back the Getpostcraft binary (your
data is forward + backward compatible) — install the previous version
from `https://github.com/thierryvm/getpostcraft/releases`. Open an
issue with screenshots so we can land a flag for the legacy renderer
in a future release if there's demand.

---

## Symptom: I uninstalled and reinstalled, my drafts are gone

Did you select "Remove user data" during uninstall? Tauri's NSIS
uninstaller doesn't ask, so this shouldn't have happened — but if
your data dir is empty:

1. `~/Documents/Getpostcraft/backups/` should still have your daily
   `.gpcbak` snapshots. Restore via Settings → Données.
2. Keychain entries for OAuth + API keys are still there — reconnect
   via Settings → Comptes.

---

## When in doubt

- Never delete `app.db` or the keychain entries before exhausting
  the options above.
- The auto-backup directory is your safety net. As long as it exists,
  worst case you lose ≤ 23 hours of work.
- File issues with the relevant `app.log` excerpt — most boot panics
  have a one-line root cause we can reproduce with a unit test.
