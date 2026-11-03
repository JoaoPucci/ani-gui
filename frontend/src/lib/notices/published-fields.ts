/** The checks behind a published notice's fields (./published-parse):
 *  text is a non-empty string, and a link a plain http or https address
 *  with no credentials. */

export const text = (v: unknown): boolean => typeof v === 'string' && v.length > 0;
export const optionalText = (v: unknown): boolean => v == null || text(v);

/** `v` as an http or https address with no credentials, or null. */
export function plainLink(v: unknown): string | null {
	if (typeof v !== 'string') return null;
	let url: URL;
	try {
		url = new URL(v);
	} catch {
		return null;
	}
	const plain =
		(url.protocol === 'http:' || url.protocol === 'https:') &&
		url.username === '' &&
		url.password === '' &&
		url.hostname !== '';
	return plain ? url.href : null;
}
