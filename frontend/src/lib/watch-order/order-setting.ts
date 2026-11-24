/**
 * The watch-order setting as the pages read it: what the stored value
 * means, and when the settings read has settled enough to draw in it.
 */
import type { WatchOrder } from './by-start';

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
