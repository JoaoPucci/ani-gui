/**
 * What the notification center's surfaces do, as pure functions the
 * components adapt: arrow keys in the panel, where opening a notice
 * goes, a notice's toast, and a row's action.
 */
import { m } from '$lib/paraglide/messages';
import type { PushArgs } from '$lib/toasts/store.svelte';
import { detailPath, toastOf, type ActionKind } from './copy';
import { toneOf, type Notice } from './model';

/** The row to focus after `key`, from row `at` (-1: outside the
 *  list) of `count`; `null` when the key is not a row key. */
export function stepRow(key: string, at: number, count: number): number | null {
	if (count === 0) return null;
	switch (key) {
		case 'ArrowDown':
			return at < 0 ? 0 : Math.min(at + 1, count - 1);
		case 'ArrowUp':
			return at < 0 ? count - 1 : Math.max(at - 1, 0);
		case 'Home':
			return 0;
		case 'End':
			return count - 1;
		default:
			return null;
	}
}

/** An element id for the row of `key`: distinct for distinct keys,
 *  made only of characters an id allows. */
export function rowDomId(key: string): string {
	const units = Array.from({ length: key.length }, (_, i) => key.charCodeAt(i).toString(36));
	return `nr-${units.join('-')}`;
}

export type View = { mode: 'page'; path: string } | { mode: 'in-place'; key: string };

/** Opening a notice goes to its page, except on the player, where
 *  nothing navigates away from the episode: it opens in the panel. */
export function viewOf(key: string, onPlayer: boolean): View {
	return onPlayer ? { mode: 'in-place', key } : { mode: 'page', path: detailPath(key) };
}

/** The toast a notice arrives with. Problems stay longer. */
export function noticeToast(n: Notice, view: (key: string) => void): PushArgs {
	const t = toastOf(n);
	const tone = toneOf(n);
	return {
		kind: t.kind,
		message: t.message,
		duration: tone === 'failure' || tone === 'live' ? 8000 : 5000,
		actionLabel: m.notices_toast_view(),
		onAction: () => view(n.key)
	};
}

export interface ActionDeps {
	openUpdate: () => void;
	reveal: (dir: string) => void;
	/** Opens an http or https address in the user's browser. */
	openLink?: (url: string) => void;
}

export function act(kind: ActionKind, n: Notice, deps: ActionDeps): void {
	if (kind === 'open-update') {
		deps.openUpdate();
		return;
	}
	if (kind === 'open-link') {
		const link = n.kind === 'remote' ? n.data.link : n.data.noteLink;
		if (typeof link === 'string' && link) deps.openLink?.(link);
		return;
	}
	const dir = n.data.destDir;
	if (typeof dir === 'string' && dir) deps.reveal(dir);
}
