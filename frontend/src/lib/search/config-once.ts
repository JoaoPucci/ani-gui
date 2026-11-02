/**
 * The search runner's settings: loaded once and shared.
 *
 * Config is cached only after a SUCCESSFUL load, and the load is
 * SHARED: concurrent runs await one in-flight load instead of racing
 * their own. Both properties close the same trap — a failure (a
 * deeplink run losing the race to the mount's settings request, or a
 * stale concurrent load rejecting after a newer one succeeded) must
 * never pin the 'sub' fallback for the mount; the next run simply
 * loads again.
 */

import type { Config } from '$lib/api';

export interface ConfigOnce {
	/** The settings last loaded; `null` until a load succeeds. */
	readonly config: Config | null;
	/** Resolves once the settings have loaded, or the load failed. */
	ready: () => Promise<void>;
}

export function configOnce(load: () => Promise<Config | null>): ConfigOnce {
	let config: Config | null = null;
	let loaded = false;
	let inFlight: Promise<void> | null = null;
	const start = () =>
		load()
			.then((c) => {
				config = c;
				loaded = true;
			})
			.catch(() => {})
			.finally(() => (inFlight = null));
	return {
		get config() {
			return config;
		},
		ready: async () => {
			if (!loaded) await (inFlight ??= start());
		}
	};
}
