/** How a row is drawn and counted: its tone, the list's order, and the
 *  bell's count and colour. */
import type { Notice, Tone } from './kinds';

export function toneOf(n: Notice): Tone {
	if (n.kind === 'download-failed') return 'failure';
	if (n.resolvedAt !== null) return 'good';
	if (n.kind === 'outage') return 'live';
	return 'info';
}

/** Display order: live conditions first, then newest first. */
export function ordered(list: readonly Notice[]): Notice[] {
	return list.slice().sort((a, b) => {
		const la = toneOf(a) === 'live' ? 0 : 1;
		const lb = toneOf(b) === 'live' ? 0 : 1;
		return la - lb || b.at - a.at;
	});
}

export interface CountOptions {
	/** The update badge shows the update, so the bell leaves it out. */
	updateOnBadge: boolean;
}

export function counted(n: Notice, opts: CountOptions): boolean {
	if (n.read) return false;
	return !(n.kind === 'update' && opts.updateOnBadge);
}

export function unreadCount(list: readonly Notice[], opts: CountOptions): number {
	return list.filter((n) => counted(n, opts)).length;
}

/** The bell's count colour: oxblood while a failure or a live
 *  condition is among the counted rows, neutral otherwise. */
export function bellTone(list: readonly Notice[], opts: CountOptions): 'attention' | 'neutral' {
	const urgent = list.some((n) => {
		const t = toneOf(n);
		return counted(n, opts) && (t === 'failure' || t === 'live');
	});
	return urgent ? 'attention' : 'neutral';
}
