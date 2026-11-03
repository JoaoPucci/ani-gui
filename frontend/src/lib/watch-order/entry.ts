/**
 * Readings of one entry: its progress as a card shows it, whether it
 * has aired, and its year.
 */
import type { EntryProgress, WatchEntry } from './types';

export type ProgressView =
	| { kind: 'none' }
	| { kind: 'watched' }
	| { kind: 'partway'; episode: number; total: number | null };

/** `progress` is the entry's own, or undefined when it has none. */
export function progressOf(entry: WatchEntry, progress: EntryProgress | undefined): ProgressView {
	if (progress === undefined) return { kind: 'none' };
	if (progress === 'watched') return { kind: 'watched' };
	return { kind: 'partway', episode: progress, total: entry.episodes };
}

/** Share of the entry watched, 0–1, for a progress bar. */
export function progressShare(view: ProgressView): number {
	if (view.kind === 'watched') return 1;
	if (view.kind === 'partway' && view.total) return Math.min(1, view.episode / view.total);
	return 0;
}

/** Not aired yet: a future start, or no start at all. */
export function isUnaired(entry: WatchEntry, today: string): boolean {
	return entry.start === null || entry.start > today;
}

/** "2023" from a start date, or null. */
export function yearOf(entry: WatchEntry): string | null {
	return entry.start?.slice(0, 4) ?? null;
}
