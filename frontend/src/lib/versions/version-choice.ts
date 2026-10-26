/**
 * The version of a show the viewer chose, and which versions a show
 * offers ("Versions of one pick" in `docs/title-resolution.md`).
 *
 * The provider lists some seasons twice — "… (Uncensored)" beside a
 * plain re-upload — and availability and play name both as versions
 * of one pick. The renderer remembers the version chosen per show,
 * keyed by its Kitsu id, and names it on every request that plays or
 * downloads the show. A request naming none plays the backend's
 * default, the uncensored copy, so nothing is stored until a choice
 * is made.
 *
 * The choices live in local storage beside the kept watch positions
 * (`$lib/play/watch-position-storage.ts`), with the same tolerances:
 * a refused or corrupt storage reads as no choice and loses only the
 * choice.
 */

import { VERSION_KINDS, type VersionKind } from '$lib/api';

/** How many shows' choices are kept; the oldest go first. */
export const MAX_CHOICES = 500;

const KEY = 'ani-gui.version-choices';

export type ChoiceStorage = Pick<Storage, 'getItem' | 'setItem'>;

/** `[kitsuId, kind]` entries, oldest write first. */
type Choices = [string, VersionKind][];

export function defaultStorage(): ChoiceStorage | null {
	try {
		return globalThis.localStorage ?? null;
	} catch {
		return null;
	}
}

const isChoice = (c: unknown): c is [string, VersionKind] =>
	Array.isArray(c) &&
	typeof c[0] === 'string' &&
	(VERSION_KINDS as readonly string[]).includes(c[1] as string);

function load(storage: ChoiceStorage | null): Choices {
	try {
		const parsed: unknown = JSON.parse(storage?.getItem(KEY) ?? '[]');
		return Array.isArray(parsed) ? parsed.filter(isChoice) : [];
	} catch {
		return [];
	}
}

/** The version chosen for the show, if one was — and so the version
 *  a request for it names; nothing leaves the backend's default. */
export function storedChoice(
	kitsuId: string,
	storage: ChoiceStorage | null = defaultStorage()
): VersionKind | undefined {
	return load(storage).find(([id]) => id === kitsuId)?.[1];
}

/** Remember `kind` for the show. Whether the storage took it; a page
 *  with no Kitsu id has nothing to remember it under. */
export function rememberChoice(
	kitsuId: string,
	kind: VersionKind,
	storage: ChoiceStorage | null = defaultStorage()
): boolean {
	if (!kitsuId || !storage) return false;
	const next: Choices = [...load(storage).filter(([id]) => id !== kitsuId), [kitsuId, kind]];
	try {
		storage.setItem(KEY, JSON.stringify(next.slice(-MAX_CHOICES)));
		return true;
	} catch {
		return false;
	}
}

/**
 * Apply the viewer's choice of `kind` for the show, and answer the
 * version requests for it name from now on. The default — the
 * uncensored copy, which a request naming none plays — is applied by
 * forgetting the choice, so requests go back to naming none rather
 * than caching the same listing under a second key. The answer is
 * what the storage holds afterwards: the play page reads the choice
 * from there, and a page playing a choice the storage refused would
 * pair its plays with that page's evictions and stamps naming another.
 */
export function applyChoice(
	kitsuId: string,
	kind: VersionKind,
	storage: ChoiceStorage | null = defaultStorage()
): VersionKind | undefined {
	if (kind === 'uncensored') forgetChoice(kitsuId, storage);
	else rememberChoice(kitsuId, kind, storage);
	return storedChoice(kitsuId, storage);
}

function forgetChoice(kitsuId: string, storage: ChoiceStorage | null): void {
	try {
		storage?.setItem(KEY, JSON.stringify(load(storage).filter(([id]) => id !== kitsuId)));
	} catch {
		// The choice stays; applyChoice answers with it.
	}
}

export { selectedVersion, versionsInMode, versionsToOffer } from './version-offer';
