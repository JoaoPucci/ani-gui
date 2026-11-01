/**
 * The captions choice the viewer made on a show, remembered per show
 * so the show's later episodes start on it.
 */

/** How many shows' choices are kept; the oldest go first. */
export const MAX_CAPTION_CHOICES = 500;

/** A remembered choice: a BCP 47 tag, or captions off. */
export type CaptionChoice = string;

export type CaptionChoiceStorage = Pick<Storage, 'getItem' | 'setItem'>;

export function defaultStorage(): CaptionChoiceStorage | null {
	try {
		return globalThis.localStorage ?? null;
	} catch {
		return null;
	}
}

/** The choice remembered for the show, if one is. */
export function storedCaption(
	kitsuId: string,
	storage: CaptionChoiceStorage | null = defaultStorage()
): CaptionChoice | undefined {
	void kitsuId;
	void storage;
	return undefined;
}

/** Remember `choice` for the show. Whether the storage took it. */
export function rememberCaption(
	kitsuId: string,
	choice: CaptionChoice,
	storage: CaptionChoiceStorage | null = defaultStorage()
): boolean {
	void kitsuId;
	void choice;
	void storage;
	return false;
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

/** The choice a pick of `target` makes, if it is one to remember. */
export function captionChoiceOf(
	list: ChoosableTrackList,
	target: number | 'off'
): CaptionChoice | undefined {
	void list;
	void target;
	return undefined;
}
