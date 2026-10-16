/**
 * Stub: the signatures the tests are written against.
 */

import type { KitsuAnimeRef, PlayArgs } from '$lib/api';

export interface PlayOwner {
	readonly showId: string;
	readonly visit: number;
	isCurrent(): boolean;
}

export function claimPlay(showId: string, visit: () => number): PlayOwner {
	void visit;
	return { showId, visit: 0, isCurrent: () => true };
}

export type PlayOutcome<S> =
	| { kind: 'played'; session: S; current: boolean }
	| { kind: 'failed'; error: unknown; message: string }
	| { kind: 'abandoned' };

export async function runOwnedPlay<S>(
	owner: PlayOwner,
	attempt: () => Promise<S>,
	describeFailure: (e: unknown) => Promise<string>
): Promise<PlayOutcome<S>> {
	void owner;
	try {
		return { kind: 'played', session: await attempt(), current: true };
	} catch (e) {
		return { kind: 'failed', error: e, message: await describeFailure(e) };
	}
}

export function playArgsFor(input: {
	title: string;
	episode: number;
	mode: 'sub' | 'dub';
	quality: string;
	show: KitsuAnimeRef | null;
	kitsuId: string | undefined;
}): PlayArgs {
	void input;
	return { title: '', episode: '', mode: 'sub', quality: '' } as PlayArgs;
}
