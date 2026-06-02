/**
 * Pick the 'sub' | 'dub' value the availability filter (and any other
 * mode-aware surface) should use, given a possibly-unloaded Config.
 *
 * Config.mode is typed as the wider `string` from the backend's TOML
 * shape, so call sites have always had to narrow it before handing
 * it to `filterAvailable` / `filterAvailableCacheOnly` /
 * `filterAvailableStrict`. The pattern got copy-pasted at 11+ sites
 * (layout, home, detail, play, topbar dropdown). Centralizing it
 * here means new surfaces inherit the right default without a chance
 * to drift, and we have one place to evolve if a third mode ever
 * shows up.
 */

import type { Config } from '$lib/api';

export function pickAvailabilityMode(config: Config | null | undefined): 'sub' | 'dub' {
	return config?.mode === 'dub' ? 'dub' : 'sub';
}
