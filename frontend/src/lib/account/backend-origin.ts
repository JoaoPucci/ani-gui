/**
 * Where the backend is, and the header that proves a request comes
 * from this app's renderer — both read from what the Electron preload
 * exposes on `window.aniGui`. Split from `./api.ts` so the HTTP-call
 * surface there stays inside the per-file complexity bar.
 */

export async function apiBase(): Promise<string> {
	const w = (typeof window !== 'undefined' ? window : undefined) as Window | undefined;
	const base = w?.aniGui?.apiBase;
	if (base) return base;
	// Fall back to vite env for browser-only dev runs.
	const env =
		typeof import.meta !== 'undefined' ? import.meta.env?.VITE_ANI_GUI_API_BASE : undefined;
	if (typeof env === 'string' && env.length > 0) return env;
	throw new Error('ani-gui apiBase is not configured');
}

export function internalSecretHeader(): Record<string, string> {
	const w = (typeof window !== 'undefined' ? window : undefined) as Window | undefined;
	const secret = w?.aniGui?.internalSecret;
	return secret ? { 'x-ani-gui-internal-secret': secret } : {};
}
