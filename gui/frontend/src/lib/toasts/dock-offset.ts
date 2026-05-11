/**
 * Pure helper for the ToastHost: where to anchor the toast stack so
 * it doesn't overlap the DownloadBar (`lib/components/DownloadBar.
 * svelte`). DownloadBar is fixed bottom-right when a download is in
 * flight; the toast must stack above it with a gap. When no
 * download is running (or the user has disabled
 * `download_bottom_bar_enabled`), the toast snaps back to its base
 * offset.
 */

export interface DockOffsetInput {
	/** Whether the DownloadBar is currently rendered. The caller
	 *  computes this from `downloadStore.hasActive &&
	 *  config.download_bottom_bar_enabled !== false`. */
	dockVisible: boolean;
	/** Base bottom offset (with no dock present), in rem.
	 *  Matches DownloadBar's own `var(--space-3)`. */
	baseRem: number;
	/** Approximate height of one DownloadBar row, in rem. Single-row
	 *  is the common case; multi-row downloads are rare enough that
	 *  a fixed clearance covers ~95% without dynamic measurement. */
	dockHeightRem: number;
	/** Visual gap between dock and toast, in rem. */
	gapRem: number;
}

export function computeToastBottomOffset(input: DockOffsetInput): number {
	if (!input.dockVisible) return input.baseRem;
	return input.baseRem + input.dockHeightRem + input.gapRem;
}
