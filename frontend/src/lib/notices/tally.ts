/** How a row is drawn and counted: its tone, the list's order, and the
 *  bell's count and colour. */
import type { Notice, Tone } from './kinds';

/** A published notice's tone, from its priority. */
const PUBLISHED_TONE: Readonly<Record<string, Tone>> = {
	critical: 'failure',
	warning: 'live'
};

export function toneOf(n: Notice): Tone {
	if (n.kind === 'remote') {
		const p = n.data.priority;
		return (
			(typeof p === 'string' && Object.hasOwn(PUBLISHED_TONE, p) && PUBLISHED_TONE[p]) || 'info'
		);
	}
	if (n.kind === 'download-failed') return 'failure';
	if (n.resolvedAt !== null) return 'good';
	if (n.kind === 'outage') return 'live';
	return 'info';
}

/** Display order: live conditions first, a critical published notice
 *  among them, then newest first. */
export function ordered(list: readonly Notice[]): Notice[] {
	const rank = (n: Notice) =>
		toneOf(n) === 'live' || (n.kind === 'remote' && toneOf(n) === 'failure') ? 0 : 1;
	return list.slice().sort((a, b) => rank(a) - rank(b) || b.at - a.at);
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
