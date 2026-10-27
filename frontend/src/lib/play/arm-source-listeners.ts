/**
 * Source-scoped listener orchestration for a media attach: seek to
 * where the episode should resume — a pending recovery's position, or
 * else where the episode was last left — mark progress for the stall
 * machine, and keep the episode's position as it plays. Everything is
 * registered with the page's source scope, which the page flushes
 * when the next stream attaches (which is also what cancels a
 * superseded attach's pending seek) and when it leaves. The play
 * page's attach path is one call into here (AGENTS.md §2).
 */

import { recoveryResume } from '$lib/play/resume-after-recovery';
import { holdForResume, resumeStartAt, type ResumeHold } from '$lib/play/resume-hold';
import { stallMachine } from '$lib/play/stall-machine';
import type { SourceScope } from '$lib/play/source-scope';
import {
	clearPosition,
	isFinishedAt,
	markStarted,
	readPosition,
	savePosition,
	type PositionStorage
} from './watch-position';

/** How far playback moves between two writes of its position. */
const SAVE_EVERY_S = 5;

/** Where the attaching stream is to start: an hls engine begins
 *  loading there. Null starts it where it starts on its own. */
export interface SourceStart {
	startAt: number | null;
}

export function armSourceScopedListeners(input: {
	video: HTMLVideoElement;
	showId: string;
	episode: number;
	scope: SourceScope;
	/** Where positions are kept; the renderer's local storage when
	 *  omitted. */
	positions?: PositionStorage;
	/** The history row whose Continue card opened the session. */
	row?: string | null;
	/** Told when the stream's picture is held back for its resume seek
	 *  and when it is revealed (`$lib/play/resume-hold`). */
	onResumeHold?: (holding: boolean) => void;
	/** Whether the stream is loaded from the start returned here — an
	 *  hls engine is; an element given a `src` opens at zero whatever
	 *  the point. */
	startedAtPoint?: boolean;
}): SourceStart {
	const { video, showId, episode, scope, positions, onResumeHold } = input;
	const row = input.row ?? null;
	// A recovery's point resumes the stream the viewer was watching a
	// moment ago, wherever it falls. A kept point was saved on an
	// earlier visit, perhaps before the stream's length was known, so
	// it is checked against the length once the metadata brings it.
	const recovered = recoveryResume.consume(showId, episode);
	const kept = recovered === null ? readPosition(showId, episode, positions) : null;
	// The episode is started from here: a play that never gets past
	// opening still leaves Continue on it.
	markStarted(showId, episode, positions, row);
	// A point to seek to holds the picture back from the attach on, so
	// the stream's first frame is never shown, nor played, before it.
	// The page's CSS can only hide the picture, and picture-in-picture
	// and a <video> that is itself fullscreen show the element directly,
	// so an hls.js stream is also started at the point (`SourceStart`): no
	// fragment before the point's is fetched, so nothing before it can
	// be shown. A started mark, at zero, has nowhere to seek.
	const startAt = resumeStartAt(recovered, kept);
	const hold: ResumeHold | null =
		startAt !== null
			? holdForResume({ video, onHold: (holding) => onResumeHold?.(holding) })
			: null;
	// Progress means frames actually rendered — the `playing` event —
	// never a bare timeupdate: the resume seek below emits one at the
	// old timestamp before the fresh source has delivered anything.
	const markProgress = () => {
		ended = false;
		stallMachine.progressed();
	};
	// The position is only the stream's once its metadata is in: an
	// element still opening reads zero, and writing that would forget
	// where the episode was left.
	let opened = false;
	// An episode played to its end is forgotten, and the source's later
	// writes — its pause, its flush — must not put it back: a length not
	// known, or a clip ending in its first seconds, would record a point
	// the end no longer judges. Playing again keeps it once more.
	let ended = false;
	let savedAt = Number.NEGATIVE_INFINITY;
	const save = () => {
		if (!opened || ended) return;
		savedAt = video.currentTime;
		savePosition(showId, episode, video.currentTime, video.duration, positions, row);
	};
	const onMetadata = () => {
		if (recovered !== null) seekTo(recovered);
		// A started mark, at zero, has nothing to judge, so only a point
		// past it waits for the length.
		if (recovered !== null || !kept || Number.isFinite(video.duration)) {
			resumeKept();
			return;
		}
		// The length is not known yet — hls.js sets it from the playlist,
		// and a playlist without an end grows it — so the kept point
		// cannot be judged. Wait for the first known length. The engine
		// seeks the element on its own, so its `seeking` does not drop
		// the wait, and while the hold is on nothing plays and the
		// player's controls are inert. Once the hold's bound has revealed
		// the stream it plays from where it is, and still nothing drops
		// the wait: the player's own seek controls need a known length.
		// Until the length comes nothing is written, so the kept point
		// outlives the wait.
		video.addEventListener('durationchange', onDuration);
	};
	const stopWaiting = () => {
		video.removeEventListener('durationchange', onDuration);
	};
	const seekTo = (seconds: number) => {
		if (hold) hold.seekTo(seconds);
		else video.currentTime = seconds;
	};
	const onDuration = () => {
		if (Number.isFinite(video.duration)) resumeKept();
	};
	// A forgotten point plays the stream from its start. A stream loaded
	// from the point is sought there, even while the element still
	// reads zero: the metadata usually comes before the point's
	// fragment is in, and the engine, which still holds the point,
	// would move the playhead back to it once that fragment is
	// buffered; a seek before then is what makes it start from zero.
	// The reveal waits for that seek like any other. Any other stream
	// is already at its start, and is revealed there at once.
	const startFromTheTop = () => {
		if (input.startedAtPoint) seekTo(0);
		else hold?.release();
	};
	// Seeks to the kept point unless the known length puts it in the
	// last 90 seconds, where it is forgotten instead. Decided once.
	const resumeKept = () => {
		stopWaiting();
		// A started mark, at zero, opens from the start and is never
		// judged finished: the episode was barely begun.
		if (recovered === null && kept !== null && kept > 0) {
			if (isFinishedAt(kept, video.duration)) {
				clearPosition(showId, episode, positions);
				startFromTheTop();
			} else seekTo(kept);
		}
		opened = true;
	};
	const onTime = () => {
		if (Math.abs(video.currentTime - savedAt) >= SAVE_EVERY_S) save();
	};
	const onEnded = () => {
		ended = true;
		clearPosition(showId, episode, positions);
	};
	scope.add(() => {
		save();
		stopWaiting();
		hold?.end();
		opened = false;
		video.removeEventListener('playing', markProgress);
		video.removeEventListener('loadedmetadata', onMetadata);
		video.removeEventListener('timeupdate', onTime);
		video.removeEventListener('pause', save);
		video.removeEventListener('ended', onEnded);
	});
	video.addEventListener('playing', markProgress);
	video.addEventListener('loadedmetadata', onMetadata, { once: true });
	video.addEventListener('timeupdate', onTime);
	video.addEventListener('pause', save);
	video.addEventListener('ended', onEnded);
	return { startAt };
}
