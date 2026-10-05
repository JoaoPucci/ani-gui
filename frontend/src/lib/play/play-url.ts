/**
 * Build the `?session=…&episode=…` query string callers append to the
 * `/play/[id]` route. Centralised so the home, detail, and prev/next
 * call sites all assemble the URL the same way — no field gets
 * silently dropped on one path while another adds it.
 */
import type { CreateSessionResponse } from '$lib/api';

/**
 * Compose the `?…` portion of a `/play/[id]` URL from a session
 * resolution + episode number. Always includes `session`, `episode`,
 * `kind`. Conditionally includes `cache_hit=1`.
 */
export function buildPlayQuery(session: CreateSessionResponse, episode: number): string {
	const parts: string[] = [
		`session=${encodeURIComponent(session.session_id)}`,
		`episode=${episode}`,
		`kind=${session.media_kind}`
	];
	if (session.cache_hit === true) {
		parts.push('cache_hit=1');
	}
	return `?${parts.join('&')}`;
}
