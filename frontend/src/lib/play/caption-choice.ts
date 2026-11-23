/**
 * The captions choice the viewer made on a show, remembered per show
 * so the show's later episodes start on it
 * ({@link import('./caption-start').startingTrack}): the picked track's
 * language tag, or Off. Picking a track whose language is unknown
 * leaves what was remembered.
 *
 * The choices live in local storage beside the version choices
 * (`$lib/versions/version-choice.ts`), keyed the same way by Kitsu id
 * and with the same tolerances: a refused or corrupt storage reads as
 * no choice and loses only the choice. Like the version choice, it is
 * the viewer's preference for the show and stays when the show leaves
 * Continue Watching.
 */

/** How many shows' choices are kept; the oldest go first. */
export const MAX_CAPTION_CHOICES = 500;

/** A remembered choice: a BCP 47 tag, or `off`. */
export type CaptionChoice = string;

export type CaptionChoiceStorage = Pick<Storage, 'getItem' | 'setItem'>;

const KEY = 'ani-gui.caption-choices';

/** `[kitsuId, choice]` entries, oldest write first. */
type Choices = [string, CaptionChoice][];

export function defaultStorage(): CaptionChoiceStorage | null {
	try {
		return globalThis.localStorage ?? null;
	} catch {
		return null;
	}
}

const isEntry = (c: unknown): c is [string, CaptionChoice] =>
	Array.isArray(c) && typeof c[0] === 'string' && typeof c[1] === 'string' && c[1] !== '';

function load(storage: CaptionChoiceStorage | null): Choices {
	try {
		const parsed: unknown = JSON.parse(storage?.getItem(KEY) ?? '[]');
		return Array.isArray(parsed) ? parsed.filter(isEntry) : [];
	} catch {
		return [];
	}
}

/** The choice remembered for the show, if one is. */
export function storedCaption(
	kitsuId: string,
	storage: CaptionChoiceStorage | null = defaultStorage()
): CaptionChoice | undefined {
	return load(storage).find(([id]) => id === kitsuId)?.[1];
}

/** Remember `choice` for the show. Whether the storage took it; a page
 *  with no Kitsu id has nothing to remember it under. */
export function rememberCaption(
	kitsuId: string,
	choice: CaptionChoice,
	storage: CaptionChoiceStorage | null = defaultStorage()
): boolean {
	if (!kitsuId || !choice || !storage) return false;
	const next: Choices = [...load(storage).filter(([id]) => id !== kitsuId), [kitsuId, choice]];
	try {
		storage.setItem(KEY, JSON.stringify(next.slice(-MAX_CAPTION_CHOICES)));
		return true;
	} catch {
		return false;
	}
}

/** What the choice reads of a text track. */
export interface ChoosableTrack {
	kind: string;
	language: string;
}

/** A TextTrackList as the choice needs it. */
export interface ChoosableTrackList {
	readonly length: number;
	[index: number]: ChoosableTrack;
}

/** The choice a pick of `target` makes, if it is one to remember: Off,
 *  or the picked subtitle track's language tag — none for a track of
 *  unknown language, a track that is not a subtitle track, or no
 *  track. */
export function captionChoiceOf(
	list: ChoosableTrackList,
	target: number | 'off'
): CaptionChoice | undefined {
	if (target === 'off') return 'off';
	if (target < 0 || target >= list.length) return undefined;
	const track = list[target];
	if (track.kind !== 'subtitles' && track.kind !== 'captions') return undefined;
	const tag = track.language.trim();
	return tag === '' || tag.toLowerCase() === 'und' ? undefined : tag;
}
