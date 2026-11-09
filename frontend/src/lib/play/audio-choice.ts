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

/** How many shows' choices are kept; the oldest go first. */
export const MAX_AUDIO_CHOICES = 500;

const KEY = 'ani-gui.audio-choices';

/** `[kitsuId, mode]` entries, oldest write first. */
type Choices = [string, AudioMode][];

const isEntry = (c: unknown): c is [string, AudioMode] =>
	Array.isArray(c) && typeof c[0] === 'string' && (c[1] === 'sub' || c[1] === 'dub');

function load(storage: AudioChoiceStorage | null): Choices {
	try {
		const parsed: unknown = JSON.parse(storage?.getItem(KEY) ?? '[]');
		return Array.isArray(parsed) ? parsed.filter(isEntry) : [];
	} catch {
		return [];
	}
}

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
	return load(storage).find(([id]) => id === kitsuId)?.[1];
}

/** Remember `mode` for the show. Whether the storage took it. */
export function rememberAudio(
	kitsuId: string,
	mode: AudioMode,
	storage: AudioChoiceStorage | null = defaultStorage()
): boolean {
	if (!kitsuId || !storage) return false;
	const next: Choices = [...load(storage).filter(([id]) => id !== kitsuId), [kitsuId, mode]];
	try {
		storage.setItem(KEY, JSON.stringify(next.slice(-MAX_AUDIO_CHOICES)));
		return true;
	} catch {
		return false;
	}
}

/** The audio a show plays in: its own choice, else `fallback`, the
 *  Settings default. */
export function audioFor(
	kitsuId: string | null | undefined,
	fallback: AudioMode,
	storage: AudioChoiceStorage | null = defaultStorage()
): AudioMode {
	return (kitsuId ? storedAudio(kitsuId, storage) : undefined) ?? fallback;
}
