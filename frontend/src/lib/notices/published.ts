/**
 * What the maintainer publishes: the notices of a signed flags file,
 * and its donation-ask number.
 *
 * The backend keeps them only from a file whose minisign signature
 * verifies, parses them strictly (backend/src/notices/remote.rs) and
 * serves them on `/api/notices` only while the notification center is
 * on. The window reads that answer strictly again, so a field that is
 * not what it should be drops its notice here too: text stays text,
 * and a link is a plain http or https address or nothing.
 *
 * A notice about the streaming source (`about: "source"`) explains the
 * outage the app works out for itself, so it joins that row rather
 * than adding one: it is shown on the outage row's page while the
 * source is not answering, and not at all otherwise.
 */
import type { NoticeData } from './model';
import type { PublishedMemory } from './published-memory';
import type { SourceChange } from './sources';

export { parsePublished } from './published-parse';

export type Priority = 'info' | 'warning' | 'critical';

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
