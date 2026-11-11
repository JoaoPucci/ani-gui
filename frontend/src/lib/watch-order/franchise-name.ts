/**
 * A franchise's display name, from the title of the entry it is named
 * after (`nameEntry`): Kitsu's format suffix ("(TV)", "(Movie)", …)
 * is dropped, unless another entry of the same franchise would end up
 * with the same name, in which case the suffix is what tells the two
 * apart and stays.
 */
import { sequence } from './sequence';
import type { WatchEntry, WatchRun } from './types';

/**
 * The entry a franchise is named after: the main story's first TV
 * series by its links, skipping the films, pilots, OVAs, ONAs and
 * specials before it; where the main story holds no TV series, its
 * first entry by its links (`run.nameId`). Link order is taken
 * whatever the order setting, so the name never changes with it.
 */
export function nameEntry(run: WatchRun): WatchEntry {
	const series = sequence(run, 'linked').find((e) => e.subtype === 'TV');
	return series ?? run.entries.find((e) => e.id === run.nameId)!;
}

/** The title without a trailing Kitsu format suffix. */
export function withoutFormatSuffix(title: string): string {
	return title.replace(/\s*\((TV|Movie|OVA|ONA|Special)\)$/i, '');
}

/**
 * `title` without its format suffix, unless one of `others` (the rest
 * of the franchise's titles) reads the same once its own suffix is
 * dropped, ignoring case.
 */
export function franchiseName(title: string, others: readonly string[] = []): string {
	const bare = withoutFormatSuffix(title);
	const key = bare.toLowerCase();
	const clash = others.some((o) => withoutFormatSuffix(o).toLowerCase() === key);
	return clash ? title : bare;
}
