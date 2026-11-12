/**
 * The launch's secret, which every request to the backend's API
 * carries.
 */

/** The header a `fetch` carries the secret in. */
export const SECRET_HEADER = 'x-ani-gui-internal-secret';

/** The query parameter the header-less routes carry it in. */
export const SECRET_QUERY = 'secret';

/** The secret as a header map: `{}` when there is none. */
export function secretHeaders(): Record<string, string> {
	return {};
}

/** `init` with the secret header added, or `init` itself when there
 *  is no secret. */
export function withSecretHeader(init?: RequestInit): RequestInit | undefined {
	return init;
}

/** `url` with the secret appended as its last query parameter, or
 *  `url` itself when there is no secret. */
export function withSecretQuery(url: string): string {
	return url;
}
