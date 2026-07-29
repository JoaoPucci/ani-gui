import type { KitsuAnimeRef } from '$lib/api';

/**
 * Second chance for a Continue Watching cap the scraper gate refused.
 *
 * The gate refuses background allmanga traffic while its breaker is
 * open. A refused availability probe still answers, but with the
 * count from the search hit rather than the per-show detail fetch —
 * and that number counts half-episodes as whole ones, so it runs one
 * high for shows carrying recaps. The backend marks such a count
 * approximate and refuses to serve it back from cache precisely so it
 * self-heals; what it needs is somebody to read again.
 *
 * Nobody did. The home page probes each row once at load, so a row
 * refused at launch kept its unconfirmed cap for the whole session,
 * and the episode its card offered could be one that does not exist.
 * Clicking was always safe — the click runs its own interactive
 * lookup, which the gate never refuses — but the card was wrong until
 * the user navigated away and back.
 *
 * The retry is shaped around how the breaker actually recovers:
 *
 *   - It stays open for a cooldown, then admits exactly ONE trial
 *     probe. So rows are retried strictly one at a time; firing them
 *     all at once spends the single trial on one row and gets the
 *     rest refused again.
 *   - The first ask is at the ordinary background spacing, not a
 *     cooldown. An unconfirmed count does not prove the gate refused
 *     — the backend falls back to the same search-hit count for any
 *     detail-fetch failure, and one transient error is far below the
 *     breaker's threshold. Assuming the worst there costs a minute of
 *     a visibly wrong card for nothing; assuming the best costs one
 *     probe the gate turns away without touching the network.
 *   - A retry that comes back unconfirmed IS a fresh refusal, so the
 *     breaker has just re-opened and later rows wait a full cooldown.
 *   - A confirmed answer proves the gate is admitting, so the
 *     remaining rows drop back to the ordinary spacing.
 *
 * Probes stay in the BACKGROUND lane. Retrying as interactive would
 * walk this traffic straight past the gate that exists to keep it
 * from poisoning the connection the user's next click depends on.
 */

/** A row whose probe came back unconfirmed, and the match to re-ask with. */
export interface ApproximateRow {
	entryId: string;
	match: KitsuAnimeRef;
}

/**
 * Accumulates the rows a retry should ask about, as their probes land.
 *
 * Which outcome earns a second ask is retry policy, so it lives here
 * rather than in the loader that happens to see the flag. The loader
 * reports every probed row and this decides: only a count that
 * arrived unconfirmed is worth re-asking. A confirmed one is final,
 * and a countless probe never produced a number to be wrong about.
 */
export function createApproximateCollector(): {
	record: (entryId: string, match: KitsuAnimeRef, approximate: boolean) => void;
	rows: ApproximateRow[];
} {
	const rows: ApproximateRow[] = [];
	return {
		record: (entryId, match, approximate) => {
			if (approximate) rows.push({ entryId, match });
		},
		rows
	};
}

export interface ApproximateRetryDeps {
	/** The loader's probe, unchanged — background lane, cache-first. */
	fetchAvailability: (
		match: KitsuAnimeRef,
		mode: 'sub' | 'dub'
	) => Promise<{ episode_count: number | null; episode_count_approximate?: boolean } | null>;
	/** Already resolved by the time the first pass finished. */
	mode: 'sub' | 'dub';
	/** Fired only for a cap the retry confirmed. */
	onRefined: (entryId: string, count: number, approximate: boolean) => void;
	/** Injected so tests don't sit out real minutes. */
	wait: (ms: number) => Promise<void>;
	/** Checked before every wait and every probe. The home route mounts
	 *  and unmounts freely and this loop outlives the first pass by
	 *  design, so it needs a way to be told to stop. */
	cancelled?: () => boolean;
	/** Lets the caller drop a row whose cap something else settled —
	 *  a click's interactive lookup is exact, so re-probing that row
	 *  spends a scraper slot to learn nothing. */
	shouldRetry?: (entryId: string) => boolean;
	/** Probes per row before giving up. An upstream that stays refused
	 *  will not be talked round by a fourth ask. */
	maxAttempts?: number;
	/** Matches the backend's `BREAKER_COOLDOWN`. */
	cooldownMs?: number;
	/** Matches the backend's `BACKGROUND_INTERVAL`. */
	pacedMs?: number;
}

const DEFAULT_MAX_ATTEMPTS = 3;
const DEFAULT_COOLDOWN_MS = 60_000;
const DEFAULT_PACED_MS = 500;

export async function retryApproximateCaps(
	rows: ApproximateRow[],
	deps: ApproximateRetryDeps
): Promise<void> {
	const maxAttempts = deps.maxAttempts ?? DEFAULT_MAX_ATTEMPTS;
	const cooldownMs = deps.cooldownMs ?? DEFAULT_COOLDOWN_MS;
	const pacedMs = deps.pacedMs ?? DEFAULT_PACED_MS;

	const queue = rows.map((row) => ({ row, attempts: 0 }));
	// Start optimistic. An unconfirmed count is NOT proof the gate
	// refused: the backend falls back to the same search-hit count
	// whenever the detail fetch fails at all, and a single transient
	// error sits well below the breaker's three-failure threshold.
	// Opening with a cooldown on that assumption would leave the card
	// wrong for a full minute while the gate was admitting throughout.
	// Escalation is driven by evidence instead, below.
	let breakerOpen = false;

	while (queue.length > 0) {
		if (deps.cancelled?.()) return;
		const job = queue.shift();
		if (!job) break;
		if (deps.shouldRetry && !deps.shouldRetry(job.row.entryId)) continue;

		await deps.wait(breakerOpen ? cooldownMs : pacedMs);
		if (deps.cancelled?.()) return;

		const answer = await probe(deps, job.row);
		job.attempts++;

		// A count with the approximate flag CLEAR is the only outcome
		// worth publishing. An approximate one is no more trustworthy
		// than what the card already shows, and a countless answer is
		// the absence of a cap — writing it would overwrite the card's
		// match.episode_count fallback with nothing.
		if (answer && answer.count != null && !answer.approximate) {
			deps.onRefined(job.row.entryId, answer.count, false);
			breakerOpen = false;
			continue;
		}

		// Only a fresh refusal is evidence the breaker re-opened. A
		// rejected or countless probe says nothing about the gate, so
		// it leaves the current assumption alone.
		if (answer?.approximate) breakerOpen = true;
		if (job.attempts < maxAttempts) queue.push(job);
	}
}

async function probe(
	deps: ApproximateRetryDeps,
	row: ApproximateRow
): Promise<{ count: number | null; approximate: boolean } | null> {
	try {
		const r = await deps.fetchAvailability(row.match, deps.mode);
		if (!r) return null;
		return { count: r.episode_count ?? null, approximate: r.episode_count_approximate === true };
	} catch {
		return null;
	}
}
