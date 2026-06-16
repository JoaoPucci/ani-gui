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
export function deriveListEntryView(entry: EntryView | null, kitsuTotal: number | null): ListEntryView {
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
	return {
		status: view.status ?? 'planning',
		progress: view.progress
	};
}
