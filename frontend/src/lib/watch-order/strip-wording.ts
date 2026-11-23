/**
 * The keys over the strip's two columns, in the wording the maintainer
 * chose: Kitsu's relation roles (Prequel, Sequel, Parent story).
 *
 * Relation roles name the link between two entries, so they hold only
 * where the strip follows links: in release order the neighbours are
 * neighbours by date, and calling an earlier-released sequel a
 * "Prequel" would be false, so the strip says "Released
 * before"/"Released after" there. A side story's parents are its
 * parents in either order.
 */
import type { StripWire, WatchOrder } from '$lib/api';

export type ColumnWord = 'prequel' | 'sequel' | 'parent' | 'released_before' | 'released_after';

export interface ColumnKey {
	readonly word: ColumnWord;
	/** The plural form ("Sequels"), where the column holds several entries. */
	readonly plural: boolean;
}

/** The key over one of the strip's columns, holding `count` entries. */
export function relationKey(
	order: WatchOrder,
	kind: 'run' | 'side',
	side: 'before' | 'after',
	count: number
): ColumnKey {
	return { word: roleWord(order, kind, side === 'before'), plural: count > 1 };
}

function roleWord(order: WatchOrder, kind: 'run' | 'side', before: boolean): ColumnWord {
	if (kind === 'side' && before) return 'parent';
	if (kind === 'run' && order === 'released') return before ? 'released_before' : 'released_after';
	return before ? 'prequel' : 'sequel';
}

/** What an empty column says. */
export type EmptyColumn = 'first' | 'last' | 'side_end';

/**
 * The text an empty column carries; null for a franchise read in part
 * (`complete` false), where the column may only be missing its entries.
 */
export function emptyColumn(
	strip: Pick<StripWire, 'kind' | 'complete'>,
	side: 'before' | 'after'
): EmptyColumn | null {
	if (!strip.complete) return null;
	if (side === 'before') return 'first';
	return strip.kind === 'side' ? 'side_end' : 'last';
}
