import { invoke } from "@tauri-apps/api/core";

export interface ConnectedAccount {
  id: number;
  provider: string;
  user_id: string;
  username: string;
  display_name: string | null;
}

export function listAccounts(): Promise<ConnectedAccount[]> {
  return invoke<ConnectedAccount[]>("list_accounts");
}

/** Opens browser, waits for callback, returns connected account. Can take up to 5 minutes. */
export function startOAuthFlow(clientId: string): Promise<ConnectedAccount> {
  return invoke<ConnectedAccount>("start_oauth_flow", { clientId });
}

export function disconnectAccount(provider: string, userId: string): Promise<void> {
  return invoke<void>("disconnect_account", { provider, userId });
}

export function saveInstagramAppId(appId: string): Promise<void> {
  return invoke<void>("save_instagram_app_id", { appId });
}

export function getInstagramAppId(): Promise<string | null> {
  return invoke<string | null>("get_instagram_app_id");
}

/** Store the Instagram app client_secret locally (never returned to renderer). */
export function saveInstagramClientSecret(secret: string): Promise<void> {
  return invoke<void>("save_instagram_client_secret", { secret });
}

/** Returns true if the client_secret has been configured. */
export function getInstagramClientSecretStatus(): Promise<boolean> {
  return invoke<boolean>("get_instagram_client_secret_status");
}
