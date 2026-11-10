/**
 * The meta line under a watch-order card, in PosterCard's voice: format,
 * year, then progress or the episode count. An unaired entry shows its
 * date (or that none is announced) in place of the year and count.
 */
import { m } from '$lib/paraglide/messages';
import { getLocale } from '$lib/paraglide/runtime';
import { subtypeLabel } from '$lib/search/subtype-label';
import { isUnaired, progressOf, yearOf } from './entry';
import type { EntryProgress, WatchEntry } from './types';

export interface MetaSegment {
	readonly text: string;
	/** `num` reads bright, `word` faint, `watched` carries the check,
	 *  `unit` is a faint word joined to the figure before it ("24 EP"). */
	readonly kind: 'num' | 'word' | 'watched' | 'unit';
}

function comingText(start: string | null): string {
	if (start === null) return m.watchorder_meta_no_date();
	const date = new Intl.DateTimeFormat(getLocale(), {
		month: 'long',
		year: 'numeric',
		timeZone: 'UTC'
	}).format(new Date(`${start}T00:00:00Z`));
	return m.watchorder_meta_coming({ date });
}

export function metaLine(
	entry: WatchEntry,
	progress: EntryProgress | undefined,
	today: string
): MetaSegment[] {
	const out: MetaSegment[] = [{ text: subtypeLabel(entry.subtype), kind: 'word' }];
	if (isUnaired(entry, today)) return [...out, { text: comingText(entry.start), kind: 'word' }];
	const year = yearOf(entry);
	if (year) out.push({ text: year, kind: 'num' });
	const p = progressOf(entry, progress);
	if (p.kind === 'watched') out.push({ text: m.watchorder_meta_watched(), kind: 'watched' });
	else if (p.kind === 'partway')
		out.push({
			text:
				p.total === null
					? m.watchorder_meta_progress_open({ episode: p.episode })
					: m.watchorder_meta_progress({ episode: p.episode, total: p.total }),
			kind: 'num'
		});
	else if (entry.episodes && entry.episodes > 1)
		out.push(
			{ text: String(entry.episodes), kind: 'num' },
			{ text: m.strip_card_episodes_suffix(), kind: 'unit' }
		);
	return out;
}

/**
 * A meta line in two parts for a card too narrow to keep it on one
 * line: the format and year, then the rest (progress, count, watched
 * or the coming date), so the second line never opens on a dot.
 */
export function splitMeta(
	segments: readonly MetaSegment[]
): [readonly MetaSegment[], readonly MetaSegment[]] {
	const yearAt = segments.findIndex((s, i) => i > 0 && s.kind === 'num' && /^\d{4}$/.test(s.text));
	const cut = yearAt === -1 ? 1 : yearAt + 1;
	return [segments.slice(0, cut), segments.slice(cut)];
}

/** An entry in a loop of `count`, said as the others it links with. */
export function loopText(count: number): string {
	return m.watchorder_strip_unnumbered_cycle({ others: count - 1 });
}

/** A loop of `count` as the heading over its group of titles. */
export function loopGroupText(count: number, complete: boolean): string {
	void complete;
	return m.watchorder_franchise_loop_group({ count });
}

/**
 * The heading over a group of ways on: their count, or no count for a
 * franchise read in part (`complete` false), where it may be short.
 */
export function waysKeyText(count: number, complete: boolean): string {
	return complete ? m.watchorder_strip_ways({ count }) : m.watchorder_upnext_choices();
}

/** "05": positions read as oversized two-digit numerals. */
export function numeral(position: number): string {
	return String(position).padStart(2, '0');
}
