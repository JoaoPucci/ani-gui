/**
 * The keys over the strip's two columns, in the four wordings the
 * mockup compares: Kitsu's relation roles (Prequel, Sequel, Parent
 * story), "Previous"/"Next", "Watch before"/"Watch after", and the
 * first round's "Before"/"After".
 *
 * Relation roles name the link between two entries, so they hold only
 * where the strip follows links: in release order the neighbours are
 * neighbours by date, and calling an earlier-released sequel a
 * "Prequel" would be false, so the role wording says "Released
 * before"/"Released after" there. A side story's parents are its
 * parents in either order.
 */
import type { WatchOrder } from './by-start';

export const STRIP_WORDINGS = ['role', 'previous', 'watch', 'plain'] as const;
export type StripWording = (typeof STRIP_WORDINGS)[number];

export type ColumnWord =
	| 'before'
	| 'after'
	| 'previous'
	| 'next'
	| 'watch_before'
	| 'watch_after'
	| 'prequel'
	| 'sequel'
	| 'parent'
	| 'released_before'
	| 'released_after';

export interface ColumnKey {
	readonly word: ColumnWord;
	/** The plural form: only relation roles have one ("Sequels"). */
	readonly plural: boolean;
}

export function columnKey(
	wording: StripWording,
	order: WatchOrder,
	kind: 'run' | 'side',
	side: 'before' | 'after',
	count: number
): ColumnKey {
	const before = side === 'before';
	if (wording === 'previous') return { word: before ? 'previous' : 'next', plural: false };
	if (wording === 'watch') return { word: before ? 'watch_before' : 'watch_after', plural: false };
	if (wording === 'plain') return { word: side, plural: false };
	return { word: roleWord(order, kind, before), plural: count > 1 };
}

function roleWord(order: WatchOrder, kind: 'run' | 'side', before: boolean): ColumnWord {
	if (kind === 'side' && before) return 'parent';
	if (kind === 'run' && order === 'released') return before ? 'released_before' : 'released_after';
	return before ? 'prequel' : 'sequel';
}
