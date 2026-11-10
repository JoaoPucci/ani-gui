/**
 * Everything the show page draws from the watch order, in one pure
 * reading: the strip itself, its caption, the masthead eyebrow's
 * position, each tile's progress, the "You're watching" card, and the
 * entries "Similar titles" leaves out because the strip shows them.
 * The page and the strip component only hand these on.
 */
import type { WatchOrder } from './by-start';
import type { WatchOrderData } from './loader';
import { loopSize } from './order';
import { progressFromHistory } from './progress';
import { stripFor } from './strip-model';
import { shownOnStrip, watchingFor, type StripView } from './strip-view';
import type { Progress, WatchEntry, WatchRun } from './types';

/** What the strip's centre says about this entry's place. */
export type StripCaption =
	| { readonly kind: 'position'; readonly position: number; readonly total: number }
	| { readonly kind: 'numeral'; readonly position: number }
	/** A numbered place in a franchise read in part: no number is sure. */
	| { readonly kind: 'partial' }
	| { readonly kind: 'side' }
	| { readonly kind: 'cycle'; readonly count: number }
	| { readonly kind: 'branch' };

export interface WatchOrderPage {
	readonly order: WatchOrder;
	readonly run: WatchRun;
	readonly view: StripView;
	/** Whether the franchise was read whole: see `StripModel.complete`. */
	readonly complete: boolean;
	readonly caption: StripCaption;
	/** "05 of 09" in the masthead eyebrow: only where the run is one line. */
	readonly eyebrow: { readonly position: number; readonly total: number } | null;
	readonly progress: Progress;
	/** The entry the viewer is part-way through, when no tile shows it. */
	readonly watching: { readonly entry: WatchEntry; readonly episode: number } | null;
	/** Ids "Similar titles" leaves out: what the strip shows. */
	readonly similarHidden: ReadonlySet<string>;
	/** The run's titles, which short titles are cut against. */
	readonly titles: readonly string[];
}

/** The order setting as stored: anything but `released` reads as linked. */
export function orderOf(value: string | null | undefined): WatchOrder {
	return value === 'released' ? 'released' : 'linked';
}

/**
 * The order to draw in once the settings read has settled (`loaded`
 * or `failed`), else null: the strip waits for it rather than
 * re-ordering under the viewer.
 */
export function settledOrder(
	config: { readonly watch_order: string } | null,
	failed: boolean
): WatchOrder | null {
	if (config) return orderOf(config.watch_order);
	return failed ? 'linked' : null;
}

/**
 * The caption for a view; `loop` is the size of the cycle `view` sits
 * in, if any. `complete` is false for a franchise read in part.
 */
export function captionOf(view: StripView, loop: number, complete: boolean): StripCaption {
	if (view.kind === 'side') return { kind: 'side' };
	if (view.position !== null && view.total !== null && complete)
		return { kind: 'position', position: view.position, total: view.total };
	if (view.position !== null) return { kind: 'numeral', position: view.position };
	return view.inCycle ? { kind: 'cycle', count: loop } : { kind: 'branch' };
}

/** The page's reading of `data` for `id`, or null where the page shows no strip. */
export function watchOrderPage(
	data: WatchOrderData,
	id: string,
	order: WatchOrder
): WatchOrderPage | null {
	const strip = stripFor(data.franchise, id, order);
	if (!strip) return null;
	const { run, view, complete } = strip;
	const progress = progressFromHistory(data.rows, run.entries);
	const watching = watchingFor(run, progress, id, view, order);
	const caption = captionOf(view, loopSize(run, id, order), complete);
	return {
		order,
		run,
		view,
		complete,
		caption,
		eyebrow:
			caption.kind === 'position' ? { position: caption.position, total: caption.total } : null,
		progress,
		watching,
		similarHidden: shownOnStrip(view, watching?.entry ?? null),
		titles: run.entries.map((e) => e.title)
	};
}
