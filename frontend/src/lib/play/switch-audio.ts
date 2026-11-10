/**
 * The Sub or Dub a switch to the picked show carries to its player
 * ($lib/play/switch-show.ts). The pick plays in its own choice, which a
 * correction gives it from the audio watched in when it had none
 * ($lib/history/carry-audio.ts). When the storage refused that choice,
 * the player the switch opens would fall back to the Settings value for
 * its next episode; the switch leaves the audio here, and the next
 * player of that show takes it once, as it mounts. A player of any
 * other show drops it, and so does a switch that did not open one.
 */

import type { AudioMode } from './audio-choice';

let pending: { showId: string; audio: AudioMode } | null = null;

/** Hand `audio` to the next player of `showId`. */
export function carrySwitchAudio(showId: string, audio: AudioMode): void {
	pending = { showId, audio };
}

/** The audio carried to `showId`, once; undefined when none. Taking
 *  clears it either way. */
export function takeSwitchAudio(showId: string): AudioMode | undefined {
	const p = pending;
	pending = null;
	return p !== null && p.showId === showId ? p.audio : undefined;
}

/** Test seam: forget any carried audio. */
export function __resetSwitchAudioForTests(): void {
	pending = null;
}
