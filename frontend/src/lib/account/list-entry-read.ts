// The detail page's live-entry read, as a function of three things it
// cannot own: how to read every connected tracker, whether the page
// still wants this read, and how to wait. The component hands those in
// and applies the outcome; the loop — attempt, classify the failure,
// plan the next attempt, wait, check the read is still wanted — lives
// here where it can be run against fakes.

import { describeError } from '$lib/play/describe-error';
import { m } from '$lib/paraglide/messages';
import { pickSeedEntry } from './list-entry-view';
import { classifyReadFailure, planReadRetry, type ReadFailure } from './read-retry';
import type { EntryView } from './types';

export type ReadOutcome =
	/** Every tracker answered; `entry` is the seed to edit from, or null
	 *  when no tracker has the show. */
	| { kind: 'ok'; entry: EntryView | null }
	/** A newer read took over while this one was in flight or waiting;
	 *  nothing from it applies. */
	| { kind: 'superseded' }
	/** The plan ran out; `last` is the failure it ended on. */
	| { kind: 'failed'; last: ReadFailure };

export interface ReadDeps {
	/** Read every connected tracker's entry; rejects on the first
	 *  tracker that cannot be read, so a partial picture never seeds. */
	readAll: () => Promise<(EntryView | null)[]>;
	/** Whether this read is still the one the page wants. */
	isCurrent: () => boolean;
	/** Wait `ms` before the next attempt. */
	sleep: (ms: number) => Promise<void>;
}

/**
 * What the list control says beside itself when a read ends: the
 * tracker's rate limit, in the backend envelope's words — "busy, try
 * again in about N s" when it named a wait — or the generic
 * rate-limit sentence when the body carried no envelope to read.
 * Nothing for any other outcome: a plain failure disables the control
 * as before, an answer needs no caption.
 */
export function readFailureNotice(outcome: ReadOutcome): string | null {
	if (outcome.kind !== 'failed' || !outcome.last.rateLimited) return null;
	const { envelope } = outcome.last;
	return envelope === null ? m.detail_list_rate_limited() : describeError(envelope);
}

/** Run the read to an outcome, retrying on the plan in `read-retry`. */
export async function readEntryWithRetry(deps: ReadDeps): Promise<ReadOutcome> {
	const failures: ReadFailure[] = [];
	for (;;) {
		try {
			const views = await deps.readAll();
			if (!deps.isCurrent()) return { kind: 'superseded' };
			return { kind: 'ok', entry: pickSeedEntry(views) };
		} catch (e) {
			if (!deps.isCurrent()) return { kind: 'superseded' };
			const failure = classifyReadFailure(e);
			failures.push(failure);
			const delay = planReadRetry(failures);
			if (delay === null) return { kind: 'failed', last: failure };
			await deps.sleep(delay);
			if (!deps.isCurrent()) return { kind: 'superseded' };
		}
	}
}
