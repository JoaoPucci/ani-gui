/**
 * What the download dock's trigger says to assistive tech.
 *
 * Pure data shape, like `bar-label.ts`: the component switches on the
 * kind and picks the Paraglide message, so this stays free of the
 * runtime and unit-testable.
 */

import type { DownloadStatus } from './store.svelte';

export type DockLabel =
	| { kind: 'active'; count: number }
	| { kind: 'unseen'; count: number }
	| { kind: 'idle' };

/** `active` is the store's downloads in progress; `unseen` its count
 *  of ended rows the user has not looked at. */
export function dockLabel(active: { status: DownloadStatus }[], unseen: number): DockLabel {
	if (active.length > 0) return { kind: 'active', count: active.length };
	if (unseen > 0) return { kind: 'unseen', count: unseen };
	return { kind: 'idle' };
}
