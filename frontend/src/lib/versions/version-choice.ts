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

import type { VersionInfo, VersionKind } from '$lib/api';

/** How many shows' choices are kept; the oldest go first. */
export const MAX_CHOICES = 500;

const KEY = 'ani-gui.version-choices';

const KINDS: readonly VersionKind[] = ['uncensored', 'standard'];

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
	Array.isArray(c) && typeof c[0] === 'string' && KINDS.includes(c[1] as VersionKind);

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

/** The versions to offer a choice between: all of them, in the
 *  provider's order, when there are several; none otherwise. */
export function versionsToOffer(versions: VersionInfo[] | null | undefined): VersionInfo[] {
	return versions && versions.length > 1 ? versions : [];
}

/** The version shown as selected: the remembered one when the show
 *  carries it, else the uncensored one — the one a request naming
 *  none plays — else the first. Nothing when nothing is offered. */
export function selectedVersion(
	offered: VersionInfo[],
	stored: VersionKind | undefined
): VersionKind | undefined {
	const kinds = offered.map((v) => v.kind);
	if (stored && kinds.includes(stored)) return stored;
	if (kinds.includes('uncensored')) return 'uncensored';
	return kinds[0];
}
