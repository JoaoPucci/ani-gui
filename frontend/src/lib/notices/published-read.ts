/**
 * The window's one read of what the maintainer published
 * (`/api/notices`, see ./published): a failed request reads as nothing
 * published; then, unless it was stopped in the meantime, the ids still
 * published are remembered, the rows raised against that memory, the
 * donation number handed to the ask, and the note about the source
 * joined to the outage rows. ./NoticeHost supplies each step and stops
 * it when it goes.
 */
import type { PublishedMemory } from './published-memory';
import { publishedChanges, readPublished, type Published } from './published';
import type { SourceChange } from './sources';

export interface PublishedReadDeps {
	read(): Promise<unknown>;
	/** ./published-memory-store `keepPublished`. */
	keepPublished(ids: readonly string[]): PublishedMemory;
	/** ./outage-note `published`. */
	joinNote(items: readonly Published[]): SourceChange[];
	take(changes: readonly SourceChange[]): void;
	setNumber(n: number | null): void;
}

/** Starts the read; the function it returns stops it. */
export function startPublishedRead(deps: PublishedReadDeps): () => void {
	let alive = true;
	void deps
		.read()
		.then(readPublished, () => readPublished(null))
		.then((p) => {
			if (!alive) return;
			const noted = deps.joinNote(p.items);
			const remembered = deps.keepPublished(p.items.map((i) => i.id));
			deps.take(publishedChanges(p.items, remembered));
			deps.setNumber(p.donationAsk);
			// An outage row raised before the answer gets its note now.
			deps.take(noted);
		});
	return () => {
		alive = false;
	};
}
