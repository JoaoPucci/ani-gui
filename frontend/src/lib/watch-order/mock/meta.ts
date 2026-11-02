/**
 * The meta line under a mockup card, in PosterCard's voice: format,
 * year, then progress or the episode count. An unaired entry shows its
 * date (or that none is announced) in place of the year and count.
 */
import { m } from '$lib/paraglide/messages';
import { getLocale } from '$lib/paraglide/runtime';
import { subtypeLabel } from '$lib/search/subtype-label';
import type { RailKey } from './order';
import type { MockEntry, MockRun } from './types';
import { isUnaired, progressOf, yearOf } from './view';

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

export function metaLine(run: MockRun, entry: MockEntry, today: string): MetaSegment[] {
	const out: MetaSegment[] = [{ text: subtypeLabel(entry.subtype), kind: 'word' }];
	if (isUnaired(entry, today)) return [...out, { text: comingText(entry.start), kind: 'word' }];
	const year = yearOf(entry);
	if (year) out.push({ text: year, kind: 'num' });
	const p = progressOf(run, entry);
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
	return [segments, []];
}

/** "05": positions read as oversized two-digit numerals. */
export function numeral(position: number): string {
	return String(position).padStart(2, '0');
}

/**
 * The key line above a rail card's title: "You are here" on the entry
 * being viewed, else what its RailKey says, else nothing.
 */
export function railKeyText(
	key: RailKey,
	here: boolean,
	label: (e: MockEntry) => string
): string | null {
	if (here) return m.watchorder_franchise_here();
	if (key === null) return null;
	if (key.kind === 'way') return m.watchorder_strip_way({ n: key.n, count: key.count });
	if (key.kind === 'loop') return m.watchorder_strip_unnumbered_cycle({ count: key.count });
	// The target's number when it has one: a title would not fit the
	// key line beside "Leads into".
	return m.watchorder_strip_leads_into({
		title: key.position !== null ? numeral(key.position) : label(key.entry)
	});
}
