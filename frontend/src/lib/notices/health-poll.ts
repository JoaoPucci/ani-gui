/**
 * Reads the source's health from the backend's loopback API while the
 * notification center runs: at start, every {@link HEALTH_POLL_MS},
 * and whenever the window regains focus. The backend answers from what
 * the app's own requests already saw (backend/src/scraper/health.rs);
 * nothing here, nor there, sends anything to the source.
 */
import { parseHealth, type ProviderHealth } from './sources';

export const HEALTH_POLL_MS = 10_000;

export interface HealthPollDeps {
	/** One read of `/api/source-health`'s JSON body. */
	read: () => Promise<unknown>;
	onHealth: (health: ProviderHealth[]) => void;
	setInterval: (fn: () => void, ms: number) => unknown;
	clearInterval: (handle: unknown) => void;
}

export interface HealthPoll {
	/** Read now, unless a read is already under way. */
	poke(): void;
	stop(): void;
}

export function startHealthPoll(deps: HealthPollDeps): HealthPoll {
	let busy = false;
	let stopped = false;
	const poke = () => {
		if (busy || stopped) return;
		busy = true;
		deps
			.read()
			.then((body) => {
				if (!stopped) deps.onHealth(parseHealth(body));
			})
			.catch(() => {
				// The backend is unreachable or answered with an error:
				// nothing is known, so nothing changes.
			})
			.finally(() => {
				busy = false;
			});
	};
	const handle = deps.setInterval(poke, HEALTH_POLL_MS);
	poke();
	return {
		poke,
		stop() {
			stopped = true;
			deps.clearInterval(handle);
		}
	};
}
