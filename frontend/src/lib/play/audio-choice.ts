/**
 * The audio (Sub or Dub) the viewer chose on a show, remembered per
 * show: every play, warm, download and hand-off of the show asks for
 * it, and its availability is read in it. A show never chosen on takes
 * the Settings value, which is only the default.
 *
 * The choices live in local storage beside the captions and version
 * choices (`./caption-choice.ts`, `$lib/versions/version-choice.ts`),
 * keyed the same way by Kitsu id and with the same tolerances: a
 * refused or corrupt storage reads as no choice and loses only the
 * choice.
 */

export type AudioMode = 'sub' | 'dub';

export type AudioChoiceStorage = Pick<Storage, 'getItem' | 'setItem'>;

export function defaultStorage(): AudioChoiceStorage | null {
	try {
		return globalThis.localStorage ?? null;
	} catch {
		return null;
	}
}

/** The audio chosen for the show, if one was. */
export function storedAudio(
	kitsuId: string,
	storage: AudioChoiceStorage | null = defaultStorage()
): AudioMode | undefined {
	void kitsuId;
	void storage;
	return undefined;
}

/** Remember `mode` for the show. Whether the storage took it. */
export function rememberAudio(
	kitsuId: string,
	mode: AudioMode,
	storage: AudioChoiceStorage | null = defaultStorage()
): boolean {
	void kitsuId;
	void mode;
	void storage;
	return false;
}

/** The audio a show plays in: its own choice, else `fallback`, the
 *  Settings default. */
export function audioFor(
	kitsuId: string | null | undefined,
	fallback: AudioMode,
	storage: AudioChoiceStorage | null = defaultStorage()
): AudioMode {
	void kitsuId;
	void storage;
	return fallback;
}
