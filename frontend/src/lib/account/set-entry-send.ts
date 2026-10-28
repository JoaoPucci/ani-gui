// One tracker's part of the list editor's fan-out: the read, then the
// write it decides on. Run as one attempt, so a write held back by a
// rate limit reads again before it goes; and asked, just before it
// writes, which of its fields are still its own — a newer write for the
// show may have covered some while it waited or while it read. Split
// from `set-entry.ts` so each file stays inside the CRAP gate's per-file
// bar.

import { AccountApiError } from './api';
import type { EntryView, ListEntry, Provider } from './types';
import type { EditorSave } from './set-entry';
import type { FanoutResult } from './set-entry-outcome';
import { buildListEdit } from './list-entry-edit';
import {
	ALL_FIELDS,
	keepWrite,
	SupersededError,
	type HeldWrite,
	type WriteField
} from './save-when-allowed';

/** Which tracker, with what bearer, for which show, and the write's
 *  view of what is still its own. */
export interface SendTo {
	provider: Provider;
	bearer: string;
	kitsuId: string;
	held: HeldWrite;
}

interface ReadDeps {
	getEntry: (provider: Provider, bearer: string, kitsuId: string) => Promise<EntryView | null>;
}

/** Read the tracker's row, decide the edit off it, and send the part of
 *  it no newer write has covered. */
export async function sendSave(
	deps: ReadDeps & {
		setEntry: (
			provider: Provider,
			bearer: string,
			body: { kitsu_id: string; status?: string; progress?: number }
		) => Promise<ListEntry | null>;
	},
	to: SendTo,
	save: EditorSave,
	fields: readonly WriteField[]
): Promise<ListEntry | null> {
	// Nothing left — newer writes took it all, or the account changed —
	// and the tracker is not read either: the read carries the bearer.
	if (to.held.left().length === 0) throw new SupersededError();
	const current = await deps.getEntry(to.provider, to.bearer, to.kitsuId);
	const edit = buildListEdit({
		current: current?.status ?? null,
		statusChanged: save.statusChanged,
		status: save.status,
		progress: save.progress,
		total: save.total ?? null
	});
	const body = keepWrite(edit, fields, to.held.left());
	if (body === null) throw new SupersededError();
	return deps.setEntry(to.provider, to.bearer, { kitsu_id: to.kitsuId, ...body });
}

/** Read the tracker's row and remove it when it is there. A removal
 *  cannot go in part: a newer write for the show that covered any field
 *  supersedes it. */
export async function sendRemoval(
	deps: ReadDeps & {
		removeEntry: (provider: Provider, bearer: string, kitsuId: string) => Promise<void>;
	},
	to: SendTo
): Promise<FanoutResult> {
	if (to.held.left().length < ALL_FIELDS.length) throw new SupersededError();
	const current = await deps.getEntry(to.provider, to.bearer, to.kitsuId);
	if (to.held.left().length < ALL_FIELDS.length) throw new SupersededError();
	try {
		await deps.removeEntry(to.provider, to.bearer, to.kitsuId);
	} catch (error) {
		// A live-null entry can also be unmappable. Only this endpoint's
		// empty 404 is neutral; upstream failures carry an error envelope.
		if (
			current === null &&
			error instanceof AccountApiError &&
			error.status === 404 &&
			error.detail === ''
		)
			return 'neither';
		throw error;
	}
	// The backend's idempotent delete also clears stale local cache.
	return current === null ? 'neither' : 'ok';
}
