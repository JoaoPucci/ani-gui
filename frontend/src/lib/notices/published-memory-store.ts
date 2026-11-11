/**
 * The memory of published notices (./published-memory) as the window
 * keeps it across launches: read from storage once, written when what
 * it remembers changes and not otherwise, and kept for the session when
 * storage is missing, unreadable or refuses the write. ./NoticeHost
 * wires it to the center's observer.
 */
import type { NoticeStorage } from './center.svelte';
import type { Notice } from './model';
import {
	PUBLISHED_STORAGE_KEY,
	keepPublished,
	parseMemory,
	remember,
	serializeMemory,
	type PublishedMemory
} from './published-memory';

export interface PublishedMemoryStore {
	/** What is remembered now. */
	readonly current: PublishedMemory;
	/** The center's list changed from `before` to `after`. */
	observe(before: readonly Notice[], after: readonly Notice[]): void;
	/** The ids the answer still publishes, moved up so they are not the
	 *  ones let go; returns the memory after. */
	keepPublished(ids: readonly string[]): PublishedMemory;
}

export function createPublishedMemoryStore(storage: NoticeStorage | null): PublishedMemoryStore {
	let memory = read(storage);
	function store(next: PublishedMemory): PublishedMemory {
		if (serializeMemory(next) === serializeMemory(memory)) return memory;
		memory = next;
		try {
			storage?.setItem(PUBLISHED_STORAGE_KEY, serializeMemory(next));
		} catch {
			// Refused: read and dismissed hold for this session only.
		}
		return memory;
	}
	return {
		get current() {
			return memory;
		},
		observe: (before, after) => void store(remember(memory, before, after)),
		keepPublished: (ids) => store(keepPublished(memory, ids))
	};
}

function read(storage: NoticeStorage | null): PublishedMemory {
	try {
		return parseMemory(storage?.getItem(PUBLISHED_STORAGE_KEY) ?? null);
	} catch {
		return parseMemory(null);
	}
}
