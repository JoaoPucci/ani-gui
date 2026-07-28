/**
 * Resolve the mode/quality a Continue Watching click should play
 * with. Cards can release (and be clicked) before settingsGet has
 * resolved — a cached Kitsu match is near-instant, settings are an
 * IPC away — and capturing the sub/best fallbacks at that instant
 * would hand a DUB user the wrong variant for both the click-time
 * cap lookup and playback. So the click awaits the same shared
 * settings promise the loader's getMode uses; the fallbacks apply
 * only when settings genuinely failed to load.
 */

import type { Config } from '$lib/api';

export async function resolveResumeSettings(
	config: Config | null,
	settingsPromise: Promise<Config | null>
): Promise<{ mode: 'sub' | 'dub'; quality: string }> {
	let c = config;
	if (!c) {
		try {
			c = await settingsPromise;
		} catch {
			c = null;
		}
	}
	return {
		mode: c?.mode === 'dub' ? 'dub' : 'sub',
		quality: c?.quality ?? 'best'
	};
}
