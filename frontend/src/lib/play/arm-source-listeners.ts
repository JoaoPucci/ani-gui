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
import { stallMachine } from '$lib/play/stall-machine';
import type { SourceScope } from '$lib/play/source-scope';
import {
	clearPosition,
	isFinishedAt,
	readPosition,
	savePosition,
	type PositionStorage
} from './watch-position';

/** How far playback moves between two writes of its position. */
const SAVE_EVERY_S = 5;

/** What the page tells the stream's listeners. */
export interface SourceListeners {
	/** The viewer moved the playhead through the page's controls: a
	 *  resume still waiting for the stream's length is dropped, and the
	 *  position is the viewer's from then on. */
	viewerMoved(): void;
}

export function armSourceScopedListeners(input: {
	video: HTMLVideoElement;
	showId: string;
	episode: number;
	scope: SourceScope;
	/** Where positions are kept; the renderer's local storage when
	 *  omitted. */
	positions?: PositionStorage;
}): SourceListeners {
	const { video, showId, episode, scope, positions } = input;
	// A recovery's point resumes the stream the viewer was watching a
	// moment ago, wherever it falls. A kept point was saved on an
	// earlier visit, perhaps before the stream's length was known, so
	// it is checked against the length once the metadata brings it.
	const recovered = recoveryResume.consume(showId, episode);
	const kept = recovered === null ? readPosition(showId, episode, positions) : null;
	// Progress means frames actually rendered — the `playing` event —
	// never a bare timeupdate: the resume seek below emits one at the
	// old timestamp before the fresh source has delivered anything.
	const markProgress = () => {
		stallMachine.progressed();
	};
	// The position is only the stream's once its metadata is in: an
	// element still opening reads zero, and writing that would forget
	// where the episode was left.
	let opened = false;
	let savedAt = Number.NEGATIVE_INFINITY;
	const save = () => {
		if (!opened) return;
		savedAt = video.currentTime;
		savePosition(showId, episode, video.currentTime, video.duration, positions);
	};
	const onMetadata = () => {
		if (recovered !== null) video.currentTime = recovered;
		if (recovered !== null || kept === null || Number.isFinite(video.duration)) {
			resumeKept();
			return;
		}
		// The length is not known yet — hls.js sets it from the playlist,
		// and a playlist without an end grows it — so the kept point
		// cannot be judged. Wait for the first known length. The element
		// autoplays and the engine seeks it on its own, so neither its
		// `playing` nor its `seeking` is the viewer: only a move the page
		// reports makes the stream theirs. Until then nothing is written,
		// and a length that never becomes known writes nothing at all.
		waiting = true;
		video.addEventListener('durationchange', onDuration);
	};
	let waiting = false;
	const stopWaiting = () => {
		waiting = false;
		video.removeEventListener('durationchange', onDuration);
	};
	const onDuration = () => {
		if (Number.isFinite(video.duration)) resumeKept();
	};
	// Seeks to the kept point unless the known length puts it in the
	// last 90 seconds, where it is forgotten instead. Decided once.
	const resumeKept = () => {
		stopWaiting();
		if (recovered === null && kept !== null) {
			if (isFinishedAt(kept, video.duration)) clearPosition(showId, episode, positions);
			else video.currentTime = kept;
		}
		opened = true;
	};
	const onTime = () => {
		if (Math.abs(video.currentTime - savedAt) >= SAVE_EVERY_S) save();
	};
	const onEnded = () => {
		clearPosition(showId, episode, positions);
	};
	scope.add(() => {
		save();
		stopWaiting();
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
	return {
		viewerMoved: () => {
			if (!waiting) return;
			stopWaiting();
			opened = true;
		}
	};
}
