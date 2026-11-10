/**
 * The Sub or Dub a correction carries — split from
 * $lib/history/link-session.svelte.ts for the per-file complexity bar.
 *
 * When the viewer corrects the show and the picked show has no Sub or
 * Dub of its own, the audio the viewer was watching in — the shown
 * show's own choice, else the Settings default as played — becomes the
 * picked show's choice ($lib/play/audio-choice.ts), so it plays in it.
 * A picked show with its own choice keeps it. No audio known yet saves
 * nothing, and a storage that refuses claims nothing: the switch then
 * plays in the audio watched in all the same.
 */

import { rememberAudio, storedAudio, type AudioChoiceStorage } from '$lib/play/audio-choice';
import type { LinkSessionDeps } from './link-session.svelte';

/** Carry the audio watched in for the show `shownId` to the picked
 *  show `pickedId`, unless it has a choice of its own. */
export function carryAudio(
	deps: Pick<LinkSessionDeps, 'audioInForce'>,
	shownId: string,
	pickedId: string,
	storage?: AudioChoiceStorage | null
): void {
	if (storedAudio(pickedId, storage) !== undefined) return;
	const audio = deps.audioInForce?.(shownId) ?? null;
	if (audio) rememberAudio(pickedId, audio, storage);
}
