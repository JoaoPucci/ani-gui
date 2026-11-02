/**
 * A card title without the words the run's entries share (the
 * proposal's open question 2): the longest leading part, ending at a
 * separator, that another member's title also starts with is dropped.
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
	return best === '' ? title : title.slice(best.length);
}
