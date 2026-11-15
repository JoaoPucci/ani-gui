/**
 * The launch's secret, which every request to the backend's API
 * carries.
 *
 * The backend listens on loopback ports any program on the machine,
 * and any page a browser on it opens, can reach. It answers `/api`
 * only to a request carrying the secret it printed at startup, which
 * the Electron main process hands to this renderer through the preload
 * bridge (`window.aniGui.internalSecret`). A `fetch` carries it as the
 * `x-ani-gui-internal-secret` header. The three routes loaded by
 * elements that cannot set a header — an `EventSource`'s progress
 * streams and an `<img>`'s `/api/image` — get the header from the
 * Electron main process, which adds it to the app's own page's
 * requests (electron/lib/secret-header.cjs). No URL carries the
 * secret: a URL is kept where a header is not, in Chromium's disk
 * cache and in any log line that names it. The stream proxy (`/s/…`)
 * needs neither: the session id in its path is its guard.
 *
 * Without a bridge (a test, browser-only dev) there is no secret, and
 * the request goes out as it is; the backend refuses it.
 */

/** The header a `fetch` carries the secret in. */
export const SECRET_HEADER = 'x-ani-gui-internal-secret';

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
