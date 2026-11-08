/**
 * A card title without the words the run's entries share (the
 * proposal's "short titles", decided in its first review round): the
 * longest leading part, ending at a separator, that another member's
 * title also starts with is dropped. With none, a trailing part that
 * another member's title starts with is dropped instead.
 * A title with no such part is kept whole.
 */
const SEPARATORS = [': ', ' - ', ' – '];

export function shortTitle(title: string, siblings: readonly string[]): string {
	let best = '';
	for (const sep of SEPARATORS) {
		let at = title.indexOf(sep);
		while (at !== -1) {
			const prefix = title.slice(0, at + sep.length);
			const shared = siblings.some((s) => s !== title && s.startsWith(prefix));
			if (shared && prefix.length > best.length && title.length > prefix.length) best = prefix;
			at = title.indexOf(sep, at + 1);
		}
	}
	if (best !== '') return title.slice(best.length);
	return withoutSharedEnd(title, siblings);
}

/** `title` without a trailing part, after a separator, that another
 *  member's title starts with as a whole word: the franchise's name
 *  set after the entry's own ("Steel Ball Run: JoJo no Kimyou na
 *  Bouken"). The longest such part is dropped; with none, the title
 *  is kept whole. */
function withoutSharedEnd(title: string, siblings: readonly string[]): string {
	let cut = title.length;
	for (const sep of SEPARATORS) {
		let at = title.indexOf(sep);
		while (at !== -1) {
			const end = title.slice(at + sep.length);
			const head = title.slice(0, at);
			const shared = siblings.some((s) => s !== title && startsWithWord(s, end));
			if (shared && head.length > 0 && at < cut) cut = at;
			at = title.indexOf(sep, at + 1);
		}
	}
	return title.slice(0, cut);
}

/** Whether `s` starts with `word` followed by its end, a space or a
 *  separator, so "JoJoland" does not start with "JoJo". */
function startsWithWord(s: string, word: string): boolean {
	if (word === '' || !s.startsWith(word)) return false;
	const next = s.slice(word.length);
	return next === '' || next.startsWith(' ') || SEPARATORS.some((sep) => next.startsWith(sep));
}
