-- Migration 020 — Security Admin (Settings → Security tab).
--
-- Foundation for the in-app security dashboard. Two tables:
--
-- `security_audit_attempts` — append-only log of password unlock attempts.
-- Feeds the lockout policy (consecutive_failures → escalating delay) and
-- gives a forensic trail if the user ever wants to see "did someone try?"
-- No PII recorded (mono-user desktop, IP fingerprints are useless).
--
-- `security_audit_reports` — landing zone for the LLM auditor outputs.
-- Phase B writes here. Declared in migration 020 (not 021) so the schema
-- ships as a coherent unit with the gate. Empty until Phase B lands.
--
-- The password hash itself is NOT stored in SQLite — it lives in the OS
-- keychain under provider name `security_password_hash` (Argon2id PHC
-- string). Reason: defense-in-depth. Even if someone reads app.db, they
-- get no hash to crack. See ADR-009 for the keychain rationale.

CREATE TABLE IF NOT EXISTS security_audit_attempts (
    id              INTEGER PRIMARY KEY,
    attempted_at    TEXT NOT NULL,        -- RFC3339 UTC
    success         INTEGER NOT NULL,     -- 0 = fail, 1 = success
    -- Free-form context. Examples: "lockout: 5s", "setup", "session_expired".
    -- Kept short (< 80 chars) on the write side.
    note            TEXT
);

CREATE INDEX IF NOT EXISTS idx_security_audit_attempts_at
    ON security_audit_attempts(attempted_at DESC);

CREATE TABLE IF NOT EXISTS security_audit_reports (
    id              INTEGER PRIMARY KEY,
    agent_name      TEXT NOT NULL,        -- "llm-security-auditor", "prompt-guardrail-auditor"
    triggered_at    TEXT NOT NULL,        -- RFC3339 UTC
    duration_ms     INTEGER,              -- NULL until run completes
    branch          TEXT,                 -- git branch at run time
    commit_sha      TEXT,                 -- git HEAD at run time
    score           REAL,                 -- 0.0-10.0, nullable on failure
    critical_count  INTEGER NOT NULL DEFAULT 0,
    high_count      INTEGER NOT NULL DEFAULT 0,
    medium_count    INTEGER NOT NULL DEFAULT 0,
    low_count       INTEGER NOT NULL DEFAULT 0,
    verdict         TEXT,                 -- "ship-ready" | "ship-with-mitigations" | "block" | "error"
    raw_report      TEXT NOT NULL,        -- full markdown body
    input_tokens    INTEGER,
    output_tokens   INTEGER,
    cost_usd        REAL,
    error           TEXT                  -- populated only when the run failed
);

CREATE INDEX IF NOT EXISTS idx_security_audit_reports_at
    ON security_audit_reports(triggered_at DESC);

CREATE INDEX IF NOT EXISTS idx_security_audit_reports_agent
    ON security_audit_reports(agent_name, triggered_at DESC);
