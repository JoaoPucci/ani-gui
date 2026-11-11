/**
 * The words for a notice: its title, the line under it, its detail
 * page's paragraphs, and its one action. Every string comes from the
 * message bundles; what a notice carries in its data (a show's title,
 * a folder, a download's own failure text, the words and link of a
 * published notice) is shown as plain text, never as markup.
 */
import { m } from '$lib/paraglide/messages';
import { getLocale } from '$lib/paraglide/runtime';
import type { ToastKind } from '$lib/toasts/store.svelte';
import { toneOf, type Notice, type Tone } from './model';
import { linkAction, linkBlock, published } from './copy-published';

export type ActionKind = 'open-update' | 'reveal-folder' | 'open-link';

export interface Action {
	kind: ActionKind;
	label: string;
}

/** One block of a detail page: an optional label over a paragraph. */
export interface Block {
	label: string | null;
	text: string;
}

export interface NoticeCopy {
	title: string;
	meta: string;
	body: Block[];
	action: Action | null;
}

const DAY_MS = 24 * 3600_000;

/** A time as a row shows it: the clock time today, the weekday and
 *  time within the week, the date before that. */
export function when(at: number, now: number = Date.now(), locale: string = getLocale()): string {
	const d = new Date(at);
	const today = new Date(now);
	const sameDay = d.toDateString() === today.toDateString();
	const time = new Intl.DateTimeFormat(locale, { hour: 'numeric', minute: '2-digit' });
	if (sameDay) return time.format(d);
	if (Math.abs(now - at) < 6 * DAY_MS) {
		return new Intl.DateTimeFormat(locale, {
			weekday: 'short',
			hour: 'numeric',
			minute: '2-digit'
		}).format(d);
	}
	return new Intl.DateTimeFormat(locale, { month: 'short', day: 'numeric' }).format(d);
}

const str = (v: unknown): string => (typeof v === 'string' ? v : '');
const ms = (v: unknown, fallback: number): number =>
	typeof v === 'number' && Number.isFinite(v) ? v : fallback;

export function copyOf(n: Notice, now: number = Date.now()): NoticeCopy {
	const d = n.data;
	switch (n.kind) {
		case 'update':
			return {
				title: m.notices_update_title({ tag: str(d.tag) }),
				meta: m.notices_update_meta({ current: str(d.current) }),
				body: [{ label: null, text: m.notices_update_body() }],
				action: { kind: 'open-update', label: m.notices_update_action() }
			};
		case 'outage':
			return n.resolvedAt === null ? outage(n, now) : recovered(n.resolvedAt, now);
		case 'download-done':
			return {
				title: m.notices_download_done_title({ count: n.count }),
				meta: str(d.title),
				body: str(d.destDir)
					? [{ label: null, text: m.notices_download_done_body({ folder: str(d.destDir) }) }]
					: [],
				action: str(d.destDir)
					? { kind: 'reveal-folder', label: m.notices_download_done_action() }
					: null
			};
		case 'remote':
			return published(n);
		case 'download-failed':
			return {
				title: m.notices_download_failed_title(),
				meta: m.notices_download_failed_meta({ title: str(d.title), episode: str(d.episode) }),
				body: [
					{
						label: m.notices_download_failed_reason_label(),
						text: str(d.reason) || m.notices_download_failed_reason_unknown()
					}
				],
				action: null
			};
	}
}

function outage(n: Notice, now: number): NoticeCopy {
	const since = when(ms(n.data.since, n.firstAt), now);
	const count = ms(n.data.failures, 0);
	const note = str(n.data.noteTitle);
	const noteText = [note, str(n.data.noteBody)].filter(Boolean).join('\n');
	return {
		title: m.notices_outage_title(),
		meta: m.notices_outage_meta({ time: since, count }),
		body: [
			{ label: null, text: m.notices_outage_body({ time: since }) },
			{ label: null, text: m.notices_outage_body_still() },
			{
				label: m.notices_outage_evidence_label(),
				text: m.notices_outage_evidence({ count, last: when(ms(n.data.last, n.at), now) })
			},
			...(note ? [{ label: m.notices_published_meta(), text: noteText }] : []),
			...linkBlock(note ? str(n.data.noteLink) : '')
		],
		action: note ? linkAction(str(n.data.noteLink)) : null
	};
}

function recovered(resolvedAt: number, now: number): NoticeCopy {
	const back = when(resolvedAt, now);
	return {
		title: m.notices_outage_resolved_title(),
		meta: m.notices_outage_resolved_meta({ time: back }),
		body: [{ label: null, text: m.notices_outage_resolved_body({ time: back }) }],
		action: null
	};
}

/** The toast a notice arrives with: amber for a live condition,
 *  oxblood for a failed event. A failure names what failed. */
export function toastOf(n: Notice): { kind: ToastKind; message: string } {
	const c = copyOf(n);
	const kinds: Record<Tone, ToastKind> = {
		failure: 'error',
		live: 'warning',
		good: 'success',
		info: 'info'
	};
	const tone = toneOf(n);
	return { kind: kinds[tone], message: tone === 'failure' ? `${c.title} · ${c.meta}` : c.title };
}

/** The detail page's address for `key`. */
export function detailPath(key: string): string {
	return `/notifications/${encodeURIComponent(key)}`;
}
