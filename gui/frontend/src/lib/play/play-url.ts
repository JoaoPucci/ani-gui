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
 * `kind`. Conditionally includes `cache_hit=1` and the resolved
 * `q` (quality) + `md` (mode).
 *
 * `quality`/`mode` describe the stream behind this session. /play reads
 * them back so the kept-alive session records the setting it was *truly*
 * resolved at — rather than inferring it from current settings, which a
 * later quality/mode change would otherwise retro-stamp. Omitted (no
 * `q`/`md`) when a caller doesn't have them.
 */
export function buildPlayQuery(
	session: CreateSessionResponse,
	episode: number,
	quality?: string,
	mode?: string
): string {
	const parts: string[] = [
		`session=${encodeURIComponent(session.session_id)}`,
		`episode=${episode}`,
		`kind=${session.media_kind}`
	];
	if (session.cache_hit === true) {
		parts.push('cache_hit=1');
	}
	if (quality) {
		parts.push(`q=${encodeURIComponent(quality)}`);
	}
	if (mode) {
		parts.push(`md=${encodeURIComponent(mode)}`);
	}
	return `?${parts.join('&')}`;
}
