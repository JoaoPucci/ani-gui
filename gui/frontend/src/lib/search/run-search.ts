/**
 * The /search page's query flow, extracted per AGENTS.md §2 so the
 * component stays a thin adapter. One runner instance owns the
 * page's async state transitions: config load, Kitsu search, and
 * the progressive availability filter's emissions.
 *
 * Superseded runs are silenced by a generation token, NOT by
 * comparing query text: navigating A → B → A starts two distinct
 * runs for the text "A", and a text guard would let the first run's
 * late emissions overwrite the second's results and clear its
 * spinner early. Every callback — results, error, busy — checks
 * that its run is still the newest before touching page state.
 */

import type { Config, KitsuAnimeRef } from '$lib/api';

export interface SearchRunnerDeps {
	kitsuSearch: (q: string) => Promise<KitsuAnimeRef[]>;
	/** Settings fetch; rejections fall back to a null config (the
	 *  mode picker defaults to 'sub'), never to the error state. */
	getConfig: () => Promise<Config | null>;
	pickMode: (config: Config | null) => 'sub' | 'dub';
	/** The progressive availability filter: emits the visible list at
	 *  the grace deadline and again on each late prune. */
	filter: (
		items: KitsuAnimeRef[],
		mode: 'sub' | 'dub',
		emit: (visible: KitsuAnimeRef[]) => void
	) => Promise<void>;
	onResults: (visible: KitsuAnimeRef[]) => void;
	onError: (e: unknown) => void;
	onBusy: (busy: boolean) => void;
}

export function createSearchRunner(deps: SearchRunnerDeps): {
	run: (q: string) => Promise<void>;
} {
	let generation = 0;
	// Config is cached only after a SUCCESSFUL load, and the load is
	// SHARED: concurrent runs await one in-flight getConfig instead
	// of racing their own. Both properties close the same trap — a
	// failure (a deeplink run losing the race to the mount's settings
	// request, or a stale concurrent load rejecting after a newer one
	// succeeded) must never pin the 'sub' fallback for the mount; the
	// next run simply loads again.
	let config: Config | null = null;
	let configLoaded = false;
	let configInFlight: Promise<void> | null = null;

	const loadConfigOnce = () => {
		configInFlight ??= deps
			.getConfig()
			.then((c) => {
				config = c;
				configLoaded = true;
			})
			.catch(() => {})
			.finally(() => {
				configInFlight = null;
			});
		return configInFlight;
	};

	const run = async (q: string) => {
		const gen = ++generation;
		const current = () => gen === generation;
		deps.onBusy(true);
		try {
			if (!configLoaded) await loadConfigOnce();
			const raw = await deps.kitsuSearch(q);
			await deps.filter(raw, deps.pickMode(config), (visible) => {
				if (!current()) return;
				deps.onResults(visible);
				deps.onBusy(false);
			});
		} catch (e) {
			if (current()) deps.onError(e);
		} finally {
			if (current()) deps.onBusy(false);
		}
	};

	return { run };
}
