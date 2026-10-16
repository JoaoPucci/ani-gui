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
	| { kind: 'active-queued'; count: number; queued: number }
	| { kind: 'unseen'; count: number }
	| { kind: 'idle' };

/** `active` is the store's downloads in progress; `unseen` its count
 *  of ended rows the user has not looked at. */
export function dockLabel(active: { status: DownloadStatus }[], unseen: number): DockLabel {
	const queued = active.filter((i) => i.status === 'queued').length;
	// Waiting downloads are named apart from the running ones: they
	// have not started, and the label must not say they are in progress.
	if (queued > 0) return { kind: 'active-queued', count: active.length - queued, queued };
	if (active.length > 0) return { kind: 'active', count: active.length };
	if (unseen > 0) return { kind: 'unseen', count: unseen };
	return { kind: 'idle' };
}
