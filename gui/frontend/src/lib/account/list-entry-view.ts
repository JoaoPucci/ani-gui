// Pure derivation for the detail-page list control: turn the live
// current entry (or its absence) + the show's episode total into the
// action-row button label and the editor's initial values. i18n is
// injected so this stays a pure, unit-tested function (the component
// passes the localized status names + "Add to list" label).

import type { EntryView, ListStatus } from './types';

/** The status options the editor offers, in display order. */
export const STATUS_OPTIONS: ListStatus[] = [
	'planning',
	'watching',
	'completed',
	'paused',
	'dropped',
	'rewatching'
];

/** Statuses that imply the show is finished — you can't have completed (or be
 *  rewatching) a title that's still airing. Hidden from the picker while the
 *  show is airing. */
const FINISHED_ONLY_STATUSES: ListStatus[] = ['completed', 'rewatching'];

/**
 * The status options to offer given whether the show is still `airing` and
 * the entry's `current` status. While airing, Completed and Rewatching are
 * dropped (you can't finish an unfinished show) — except a status the entry
 * is *already* set to (e.g. completed on another client) is kept, so the
 * editor reflects reality and doesn't silently downgrade it.
 */
export function statusOptionsFor(airing: boolean, current: ListStatus | null): ListStatus[] {
	if (!airing) return STATUS_OPTIONS;
	return STATUS_OPTIONS.filter((s) => !FINISHED_ONLY_STATUSES.includes(s) || s === current);
}

// How "committed" each status is, for breaking ties when two trackers
// sit at the same episode count. A higher rank means further along the
// watch lifecycle, so on a tie we seed from the more-engaged entry.
const STATUS_COMMITMENT: Record<ListStatus, number> = {
	planning: 0,
	dropped: 1,
	paused: 2,
	watching: 3,
	rewatching: 4,
	completed: 5
};

/**
 * Fold the live current entry read from every connected provider into the
 * single entry the editor seeds from. The detail page reads each tracker
 * but a Save fans the result out to ALL of them, so seeding from one
 * provider that lacks the title (→ Add/Planning/0) would let a Save clobber
 * another tracker that already has progress. Picking the furthest-along
 * entry — greatest progress, tie-broken by the more-committed status —
 * means writing the seed back never lowers a tracker below where it was.
 * Returns null only when no provider has the show on its list.
 */
export function pickSeedEntry(entries: (EntryView | null)[]): EntryView | null {
	let best: EntryView | null = null;
	for (const e of entries) {
		if (!e) continue;
		if (
			best === null ||
			e.progress > best.progress ||
			(e.progress === best.progress && STATUS_COMMITMENT[e.status] > STATUS_COMMITMENT[best.status])
		) {
			best = e;
		}
	}
	return best;
}

export interface ListEntryView {
	/** Whether the show is on the user's list. */
	onList: boolean;
	/** Current unified status, or null when not on the list. */
	status: ListStatus | null;
	/** Episodes watched so far (0 when not on the list). */
	progress: number;
	/** The show's full episode total, or null when unknown. */
	total: number | null;
}

/**
 * Fold the live `EntryView` (from `getEntry`, `null` when not on the
 * list) + the Kitsu episode total into the view model the action row and
 * editor consume.
 */
export function deriveListEntryView(
	entry: EntryView | null,
	kitsuTotal: number | null
): ListEntryView {
	if (!entry) {
		return { onList: false, status: null, progress: 0, total: kitsuTotal };
	}
	return { onList: true, status: entry.status, progress: entry.progress, total: kitsuTotal };
}

export interface ListButtonLabels {
	/** Label when the show isn't on the list yet (e.g. "Add to list"). */
	add: string;
	/** Localized name for a status (e.g. 'watching' → "Watching"). */
	statusLabel: (status: ListStatus) => string;
}

/**
 * The action-row button text: the add label when not on the list, else
 * "Status · n/total" (or "Status · n" when the total is unknown).
 */
export function listButtonLabel(view: ListEntryView, labels: ListButtonLabels): string {
	if (!view.onList || view.status === null) return labels.add;
	const count = view.total !== null ? `${view.progress}/${view.total}` : `${view.progress}`;
	return `${labels.statusLabel(view.status)} · ${count}`;
}

/**
 * Initial editor values: seed from the live entry when on the list,
 * else default to a fresh Plan-to-Watch at episode 0.
 */
export function editorInitial(view: ListEntryView): { status: ListStatus; progress: number } {
	const status = view.status ?? 'planning';
	// A completed entry opens at the full count, never a stored-short value —
	// the editor then locks the episode field while Completed is selected.
	return { status, progress: effectiveProgress(status, view.progress, view.total) };
}

/**
 * Build the per-tracker edit the editor's Save sends to one connected
 * tracker, given that tracker's own current status (`current`, null when it
 * doesn't have the row) and the show's episode `total`.
 *
 * The target status for the tracker is the user's pick when they're creating
 * the row or deliberately changed status off the seed; otherwise we preserve
 * the tracker's own status (the seed collapses divergent trackers to one, so
 * blindly writing it back would wipe a deliberate rewatching/paused/dropped
 * state elsewhere). That target is then made coherent with the progress —
 * `planning` with watched episodes becomes `watching`; `completed` always
 * carries the full count ([`effectiveProgress`]) — and is judged against the
 * tracker's *real* current status, not the app's (possibly stale) seed, so a
 * provider that's actually completed never ends up at completed/0.
 *
 * Status is sent only when the coherent target differs from the tracker's
 * current (or the row is new); progress always rides along.
 */
export function buildListEdit(opts: {
	current: ListStatus | null;
	seededStatus: ListStatus;
	status: ListStatus;
	progress: number;
	total?: number | null;
}): { status?: ListStatus; progress: number } {
	const total = opts.total ?? null;
	// The user's pick wins when creating the row or when they moved status off
	// the seed; otherwise keep this tracker's own status.
	const target =
		opts.current === null || opts.status !== opts.seededStatus ? opts.status : opts.current;
	const status = effectiveStatus(target, opts.progress);
	const edit: { status?: ListStatus; progress: number } = {
		progress: effectiveProgress(status, opts.progress, total)
	};
	if (opts.current === null || status !== opts.current) {
		edit.status = status;
	}
	return edit;
}

/**
 * The status actually worth writing for a given chosen status + progress:
 * `planning` with watched episodes is incoherent (and keeps the title in
 * Watch Later), so a started title is `watching`. Everything else is left
 * as chosen. Shared by the write fan-out and the editor's optimistic
 * post-save state so they agree on the promoted status.
 */
export function effectiveStatus(status: ListStatus, progress: number): ListStatus {
	return status === 'planning' && progress > 0 ? 'watching' : status;
}

/**
 * The episode count to write for a status: a `completed` entry always carries
 * the full count — you can't be completed with fewer (status wins over a
 * partial episode edit), and writing completed/0 is incoherent. Every other
 * status keeps the given count. Left untouched when the total is unknown.
 * Shared by the write fan-out and the editor (which locks the episode field
 * to the total while Completed is selected).
 */
export function effectiveProgress(
	status: ListStatus,
	progress: number,
	total: number | null
): number {
	return status === 'completed' && total !== null ? total : progress;
}

/**
 * Clamp an episode count the editor is about to set: floor at 0, coerce a
 * non-finite entry (empty/NaN input) to 0, floor a fraction to a whole
 * episode, and — when the show's total is known — cap at it, so the user
 * can't confirm an episode the show doesn't have. The total is left
 * uncapped when unknown (an ongoing show whose count metadata is missing).
 */
export function clampProgress(value: number, total: number | null): number {
	if (!Number.isFinite(value) || value < 0) return 0;
	const whole = Math.floor(value);
	return total !== null && whole > total ? total : whole;
}
