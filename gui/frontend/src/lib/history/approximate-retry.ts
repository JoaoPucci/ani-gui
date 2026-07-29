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
 *   - Each row climbs a fixed backoff ladder rather than reacting to
 *     what its answers looked like. Nothing in the response says
 *     whether the gate refused, so the ladder is sized to cover both
 *     the transient case and the breaker cooldown — see `BACKOFF_MS`.
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
	/**
	 * How long a row waits before each of its attempts, indexed by
	 * attempts already made; the last entry repeats. A fixed ladder
	 * rather than a reaction to the answers, because nothing in the
	 * response says whether the gate refused — see the note above
	 * `BACKOFF_MS`.
	 */
	backoffMs?: number[];
}

/**
 * The ladder. Its shape is set by what it has to cover, not by what
 * any single answer means:
 *
 *   - 500 ms — the backend's `BACKGROUND_INTERVAL`. Catches the
 *     common case, a detail fetch that failed once for ordinary
 *     transient reasons with the gate admitting throughout.
 *   - 30 s, then 60 s — the backend's `BREAKER_COOLDOWN` is 60 s, so
 *     by the third attempt a row that really was gate-refused is past
 *     it. The two earlier attempts cost nothing in that case: a
 *     refusal skips the network entirely by design.
 *
 * Read the other way round, this is why the schedule cannot be
 * derived from the answers. `episode_count_approximate` means "this
 * count came from the search hit", which the backend sets for a
 * gate refusal AND for any failed detail fetch — and the breaker
 * only opens after three consecutive failures, so one or two
 * transient errors set the flag with background traffic still
 * flowing. Treating it as proof of an open breaker delays the
 * correction by a minute for nothing; treating its absence as proof
 * of a closed one hammers a gate that is refusing. The ladder claims
 * neither.
 */
const BACKOFF_MS = [500, 30_000, 60_000];

export async function retryApproximateCaps(
	rows: ApproximateRow[],
	deps: ApproximateRetryDeps
): Promise<void> {
	const backoff = deps.backoffMs ?? BACKOFF_MS;
	const maxAttempts = deps.maxAttempts ?? backoff.length;

	const queue = rows.map((row) => ({ row, attempts: 0 }));

	while (queue.length > 0) {
		if (deps.cancelled?.()) return;
		const job = queue.shift();
		if (!job) break;
		if (skip(deps, job.row.entryId)) continue;

		await deps.wait(backoff[Math.min(job.attempts, backoff.length - 1)]);
		if (deps.cancelled?.()) return;
		// Again, on the way out. The later steps are half a minute and
		// a minute long, and a click landing in that window pins an
		// exact cap through its own interactive lookup — which is the
		// condition the skip exists to detect. Asking only on the way
		// in spends a scraper slot on a question already answered, and
		// a failure there counts toward the breaker.
		if (skip(deps, job.row.entryId)) continue;

		const answer = await probe(deps, job.row);
		job.attempts++;

		// A count with the approximate flag CLEAR is the only outcome
		// worth publishing. An approximate one is no more trustworthy
		// than what the card already shows, and a countless answer is
		// the absence of a cap — writing it would overwrite the card's
		// match.episode_count fallback with nothing.
		if (answer && answer.count != null && !answer.approximate) {
			deps.onRefined(job.row.entryId, answer.count, false);
			continue;
		}

		if (job.attempts < maxAttempts) queue.push(job);
	}
}

/** The caller's click-settled check, absent-means-retry. */
function skip(deps: ApproximateRetryDeps, entryId: string): boolean {
	return deps.shouldRetry ? !deps.shouldRetry(entryId) : false;
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
