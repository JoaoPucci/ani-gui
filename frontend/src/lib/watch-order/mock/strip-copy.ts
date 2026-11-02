/**
 * The strip's copy for the readings in `./strip-view` and
 * `./strip-wording`: Paraglide messages only, so the components stay
 * thin.
 */
import type { EmptyColumn } from './strip-view';
import type { ColumnKey } from './strip-wording';

export function columnKeyText(key: ColumnKey): string {
	void key;
	return '';
}

/**
 * The line under a key where a column holds several entries: a join or
 * a fork. A side story's several parents are neither, and get none.
 */
export function columnCountText(
	kind: 'run' | 'side',
	side: 'before' | 'after',
	count: number
): string | null {
	void kind;
	void side;
	void count;
	return null;
}

export function emptyColumnText(kind: EmptyColumn): string {
	void kind;
	return '';
}
