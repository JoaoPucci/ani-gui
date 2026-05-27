/**
 * DownloadBar adaptive-label helper.
 *
 * Pure data shape — returns a discriminated union the Svelte
 * template switches on to pick the matching Paraglide message.
 * Keeping the i18n call site in the component means this helper
 * has zero dependencies and stays unit-testable.
 *
 * See `bar-label.test.ts` for the exhaustive expected outputs.
 */

export interface DownloadBarItem {
	title: string;
	/** Episode argument as sent to ani-cli — `"5"` for single,
	 *  `"5-12"` for range. */
	episode: string;
	rangeTotal: number | null;
	currentEp: number | null;
}

export type DownloadBarLabel =
	| { kind: 'single'; title: string; episode: string }
	| { kind: 'single-range-progress'; title: string; currentEp: number; total: number }
	| { kind: 'single-range-static'; title: string; episode: string }
	| { kind: 'multi'; episodeCount: number };

export function formatDownloadBarLabel(items: DownloadBarItem[]): DownloadBarLabel | null {
	if (items.length === 0) return null;
	if (items.length === 1) {
		const it = items[0];
		if (it.rangeTotal && it.currentEp !== null) {
			return {
				kind: 'single-range-progress',
				title: it.title,
				currentEp: it.currentEp,
				total: it.rangeTotal
			};
		}
		if (it.rangeTotal) {
			return { kind: 'single-range-static', title: it.title, episode: it.episode };
		}
		return { kind: 'single', title: it.title, episode: it.episode };
	}
	const episodeCount = items.reduce((sum, it) => sum + (it.rangeTotal ?? 1), 0);
	return { kind: 'multi', episodeCount };
}
