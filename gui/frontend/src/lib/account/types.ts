/**
 * Mirror of the Rust backend's `account/provider.rs` types.
 *
 * Names are snake_case where the wire format is snake_case, mixed
 * with camelCase where the frontend uses them directly — same
 * convention as `lib/api.ts`.
 */

export type Provider = 'anilist' | 'mal' | 'inhouse';

export type ListStatus =
	| 'planning'
	| 'watching'
	| 'completed'
	| 'paused'
	| 'dropped'
	| 'rewatching';

export interface Tokens {
	access_token: string;
	refresh_token: string | null;
	expires_at_epoch_s: number;
}

export interface UserStats {
	anime_count: number;
	mean_score_0_to_10: number | null;
}

export interface UserProfile {
	provider: Provider;
	user_id: string;
	username: string;
	avatar_url: string | null;
	stats: UserStats | null;
}

export interface ListEntry {
	provider: Provider;
	media_id: number;
	mal_id: number | null;
	status: ListStatus;
	progress_episodes: number;
	score_0_to_100: number | null;
	updated_at_epoch_s: number;
	title: string;
}

/**
 * The blob we hand to Electron's safeStorage. Combines the OAuth
 * tokens (returned by the backend's `/api/account/exchange-code`)
 * with the user_id from the subsequent `/me` call so the renderer
 * can do cache reads without an extra round trip on every launch.
 */
export interface PersistedAccount {
	access_token: string;
	refresh_token: string | null;
	expires_at_epoch_s: number;
	user_id: string;
	username: string;
	avatar_url: string | null;
}

/**
 * Per-provider state in the frontend store. `error` covers transient
 * failures (last sync failed); `expired` is the AniList 1-year JWT
 * scenario where a fresh login is required.
 */
export type ProviderState =
	| { kind: 'disconnected' }
	| { kind: 'connecting' }
	| { kind: 'connected'; account: PersistedAccount; lastSyncedAt: number | null }
	| { kind: 'expired'; account: PersistedAccount }
	| { kind: 'error'; account: PersistedAccount | null; message: string };

/**
 * PKCE pair as the renderer keeps it locally between auth-url and
 * exchange-code. Matches the backend's `api/account.rs PkceWire`.
 */
export interface PkceWire {
	verifier: string;
	challenge: string;
	method: 'plain' | 'S256';
}

/**
 * Electron preload surface exposed under `window.aniGui.account.*`.
 * Declared here (vs. in lib/api.ts) so the account module is
 * self-contained.
 */
export interface OAuthOpenArgs {
	authUrl: string;
}
export type OAuthOpenResult =
	| { ok: true; code: string; state: string }
	| { ok: false; kind: string; message?: string };

export type SetTokenResult = { ok: true } | { ok: false; kind: string; message?: string };
export type GetTokenResult =
	| { ok: true; payload: PersistedAccount }
	| { ok: false; kind: string; message?: string };

export interface AniGuiAccountBridge {
	openOAuth(args: OAuthOpenArgs): Promise<OAuthOpenResult>;
	cancelOAuth(): Promise<boolean>;
	setToken(provider: Provider, payload: PersistedAccount): Promise<SetTokenResult>;
	getToken(provider: Provider): GetTokenResult;
	clearToken(provider: Provider): Promise<{ ok: boolean; kind?: string; message?: string }>;
}
