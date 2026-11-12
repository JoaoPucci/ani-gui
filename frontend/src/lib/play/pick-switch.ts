/**
 * The lifecycle of "Switch to it now" on the player: what happens
 * around the switch itself ($lib/play/switch-show.ts).
 *
 * One switch at a time, and none while another play of the page holds
 * the switch lock. Where playback stands is taken first — the point,
 * whether it was playing, the speed — and the video is paused for the
 * between-episodes band. The pick plays in its own Sub or Dub, or the
 * session's when it has none stored ($lib/history/carry-audio.ts), and
 * that audio is handed to the next player of the pick
 * ($lib/play/switch-audio.ts) before the switch runs; whatever the
 * switch did, nothing is left carried once it is over, since the pick's
 * player took it as it mounted. A switch that failed — one whose own
 * effects threw among them — says so, and the
 * episode on screen goes on as it was: playing again if it was playing.
 * One abandoned — the viewer left — does neither.
 *
 * The page is an adapter: it hands in the video, the lock, the audio
 * stores and the switch with its own effects.
 */

import type { KitsuAnimeRef, VersionKind } from '$lib/api';
import type { AudioMode } from './audio-choice';
import type { SwitchOutcome, SwitchRequest } from './switch-show';

/** The show picked for the card, while the player still plays the one
 *  before. */
export interface Pick {
	id: string;
	title: string;
	show: KitsuAnimeRef;
}

/** Where playback stands as the switch begins. */
export interface Playback {
	at: number;
	playing: boolean;
	rate: number;
}

export interface PickSwitchDeps {
	/** Whether a switch, or another play of the page, holds the lock. */
	busy: () => boolean;
	setBusy: (busy: boolean) => void;
	playback: () => Playback;
	pause: () => void;
	/** Play the episode on screen again. */
	resume: () => void;
	/** The pick's own stored Sub or Dub, if any. */
	storedAudio: (showId: string) => AudioMode | undefined;
	/** The Sub or Dub the session plays in. */
	sessionAudio: () => AudioMode;
	carryAudio: (showId: string, audio: AudioMode) => void;
	takeAudio: (showId: string) => void;
	/** The version and captions remembered for the pick. */
	choices: (showId: string) => { version?: VersionKind; caption?: string };
	/** The episode playing, and the card row the session is for. */
	episode: () => number;
	row: () => string | null;
	/** The switch itself, with the page's effects. */
	run: (req: SwitchRequest) => Promise<SwitchOutcome>;
	/** Say the switch to `pick` failed. */
	failed: (pick: Pick) => void;
}

/** Switch to `pick`; what came of it, or `skipped` when there was no
 *  pick or the lock was held. */
export async function switchToPick(
	pick: Pick | null,
	deps: PickSwitchDeps
): Promise<SwitchOutcome | 'skipped'> {
	if (!pick || deps.busy()) return 'skipped';
	const playback = deps.playback();
	deps.pause();
	const mode = deps.storedAudio(pick.id) ?? deps.sessionAudio();
	deps.carryAudio(pick.id, mode);
	const { version, caption } = deps.choices(pick.id);
	deps.setBusy(true);
	let outcome: SwitchOutcome;
	try {
		outcome = await deps.run({
			to: pick.show,
			episode: deps.episode(),
			at: playback.at,
			row: deps.row(),
			mode,
			version,
			caption,
			rate: playback.rate
		});
	} catch {
		// A switch whose own effects threw switched to nothing.
		outcome = 'failed';
	} finally {
		deps.takeAudio(pick.id);
		deps.setBusy(false);
	}
	if (outcome === 'failed') {
		deps.failed(pick);
		if (playback.playing) deps.resume();
	}
	return outcome;
}
