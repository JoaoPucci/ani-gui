/**
 * The launch's secret, which every request to the backend's API
 * carries.
 *
 * The backend listens on loopback ports any program on the machine,
 * and any page a browser on it opens, can reach. It answers `/api`
 * only to a request carrying the secret it printed at startup, which
 * the Electron main process hands to this renderer through the preload
 * bridge (`window.aniGui.internalSecret`). A `fetch` carries it as the
 * `x-ani-gui-internal-secret` header; the three routes loaded by
 * elements that cannot set a header — an `EventSource`'s progress
 * streams and an `<img>`'s `/api/image` — carry it as the `secret`
 * query parameter. The stream proxy (`/s/…`) needs neither: the
 * session id in its path is its guard.
 *
 * Without a bridge (a test, browser-only dev) there is no secret, and
 * the request goes out as it is; the backend refuses it.
 */

/** The header a `fetch` carries the secret in. */
export const SECRET_HEADER = 'x-ani-gui-internal-secret';

/** The query parameter the header-less routes carry it in. */
export const SECRET_QUERY = 'secret';

function bridgeSecret(): string | null {
	if (typeof window === 'undefined') return null;
	const secret = window.aniGui?.internalSecret;
	return typeof secret === 'string' && secret.length > 0 ? secret : null;
}

/** The secret as a header map: `{}` when there is none. */
export function secretHeaders(): Record<string, string> {
	const secret = bridgeSecret();
	return secret === null ? {} : { [SECRET_HEADER]: secret };
}

/** `init` with the secret header added, or `init` itself when there
 *  is no secret. */
export function withSecretHeader(init?: RequestInit): RequestInit | undefined {
	const secret = bridgeSecret();
	if (secret === null) return init;
	const headers = { ...((init?.headers as Record<string, string> | undefined) ?? {}) };
	headers[SECRET_HEADER] = secret;
	return { ...(init ?? {}), headers };
}

/** `url` with the secret appended as its last query parameter, or
 *  `url` itself when there is no secret. */
export function withSecretQuery(url: string): string {
	const secret = bridgeSecret();
	if (secret === null) return url;
	const sep = url.includes('?') ? '&' : '?';
	return `${url}${sep}${SECRET_QUERY}=${encodeURIComponent(secret)}`;
}
