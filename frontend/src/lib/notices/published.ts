/**
 * What the maintainer publishes: the notices of a signed flags file,
 * and its donation-ask number.
 *
 * The backend keeps them only from a file whose minisign signature
 * verifies, validates them (backend/src/notices/remote.rs) and serves
 * them on `/api/notices` only while the notification center is on. It
 * is the one place a notice is validated: what it serves is plain text,
 * a link a plain http or https address with no credentials or nothing,
 * one id once. The window checks only that the answer is the
 * documented object with a list, and reads anything else, or a failed
 * request, as nothing published.
 *
 * A notice about the streaming source (`about: "source"`) explains the
 * outage the app works out for itself, so it joins that row rather
 * than adding one: it is shown on the outage row's page while the
 * source is not answering, and not at all otherwise.
 */
import type { NoticeData } from './model';
import type { PublishedMemory } from './published-memory';
import type { SourceChange } from './sources';

export type Priority = 'info' | 'warning' | 'critical';

/** One notice as `/api/notices` serves it: `RemoteNotice` in
 *  backend/src/notices/remote.rs. */
export interface ServedNotice {
	id: string;
	title: string;
	body: string | null;
	link: string | null;
	priority: Priority;
	about_source: boolean;
}

/** The `/api/notices` answer: `RemoteNotices` in
 *  backend/src/notices/remote.rs. */
export interface ServedAnswer {
	donation_ask: number | null;
	items: ServedNotice[];
}

export interface Published {
	id: string;
	title: string;
	body: string | null;
	link: string | null;
	priority: Priority;
	aboutSource: boolean;
}

export interface PublishedAnswer {
	/** The donation-ask number, or null when none is published. */
	donationAsk: number | null;
	items: Published[];
}

const NOTHING: PublishedAnswer = { donationAsk: null, items: [] };

/** The backend's answer, when it is the documented object with a list
 *  of notices; anything else, and a request that failed (which
 *  ./NoticeHost passes in as null), reads as nothing published. A
 *  served notice is read as it is: only a missing priority or source
 *  mark takes the answer's documented default. */
export function readPublished(raw: unknown): PublishedAnswer {
	if (typeof raw !== 'object' || raw === null || !Array.isArray((raw as ServedAnswer).items)) {
		return NOTHING;
	}
	const answer = raw as ServedAnswer;
	return {
		donationAsk: answer.donation_ask ?? null,
		items: answer.items
			.filter((n) => typeof n === 'object' && n !== null)
			.map((n) => ({
				id: n.id,
				title: n.title,
				body: n.body ?? null,
				link: n.link ?? null,
				priority: n.priority ?? 'info',
				aboutSource: n.about_source === true
			}))
	};
}

export const publishedKey = (id: string): string => `remote:${id}`;

/** The rows the published notices raise: one each, except those about
 *  the source and those dismissed at an earlier launch; one read then is
 *  raised read. */
export function publishedChanges(
	items: readonly Published[],
	memory: PublishedMemory
): SourceChange[] {
	return items
		.filter((p) => !p.aboutSource && !memory.dismissed.includes(p.id))
		.map((p) => ({
			op: 'raise',
			input: {
				key: publishedKey(p.id),
				kind: 'remote',
				read: memory.read.includes(p.id),
				data: { title: p.title, body: p.body, link: p.link, priority: p.priority }
			}
		}));
}

/** What a notice about the source adds to the outage row. */
export interface SourceNote {
	title: string;
	body: string | null;
	link: string | null;
}

/** The first notice published about the source, or null. */
export function sourceNote(items: readonly Published[]): SourceNote | null {
	const p = items.find((i) => i.aboutSource);
	return p ? { title: p.title, body: p.body, link: p.link } : null;
}

/** `changes` with the note joined to every outage row they raise. */
export function withSourceNote(
	changes: readonly SourceChange[],
	note: SourceNote | null
): SourceChange[] {
	if (!note) return [...changes];
	const extra: NoticeData = { noteTitle: note.title, noteBody: note.body, noteLink: note.link };
	return changes.map((c) =>
		c.op === 'raise' && c.input.kind === 'outage'
			? { ...c, input: { ...c.input, data: { ...c.input.data, ...extra } } }
			: c
	);
}
