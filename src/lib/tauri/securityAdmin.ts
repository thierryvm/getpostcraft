import { invoke } from "./invoke";

/**
 * Tagged-union mirror of `commands::security_admin::VerifyResponse` on
 * the Rust side. Renderer pattern-matches on `kind` to render the right
 * UI state (open the gate, show lockout countdown, redirect to setup,
 * or surface a "wrong password" error).
 */
export type VerifyResponse =
  | { kind: "ok"; token: string }
  | { kind: "locked_out"; wait_seconds: number }
  | { kind: "no_password_set" }
  | { kind: "wrong" };

export interface SecurityAttemptRow {
  id: number;
  attempted_at: string;
  /** SQLite stores BOOLEAN as INTEGER, so the wire shape is 0|1. */
  success: number;
  note: string | null;
}

export async function isSecurityPasswordSet(): Promise<boolean> {
  return invoke<boolean>("is_security_password_set");
}

export async function setupSecurityPassword(plain: string): Promise<void> {
  return invoke<void>("setup_security_password", { plain });
}

export async function verifySecurityPassword(
  plain: string,
): Promise<VerifyResponse> {
  return invoke<VerifyResponse>("verify_security_password", { plain });
}

export async function checkSecuritySession(token: string): Promise<boolean> {
  return invoke<boolean>("check_security_session", { token });
}

export async function endSecuritySession(): Promise<void> {
  return invoke<void>("end_security_session");
}

export async function listRecentSecurityAttempts(
  limit = 50,
): Promise<SecurityAttemptRow[]> {
  return invoke<SecurityAttemptRow[]>("list_recent_security_attempts", { limit });
}
