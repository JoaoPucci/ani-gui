/**
 * hls.js's own subtitle selection, kept in step with the captions the
 * app shows, on a stream whose playlist carries subtitle renditions.
 *
 * The signatures only, for now: every pick still sets the tracks alone,
 * as it did before.
 */

import type Hls from 'hls.js';
import { applyCaptionPick, type CaptionMode, type CaptionTarget } from './caption-pick';

/** A rendition as the engine lists it. */
export interface EngineTrack {
	name: string;
	lang?: string;
}

/** What hls.js reads of a text track to tell whose it is. */
export interface NamedTrack {
	readonly label: string;
	readonly language: string;
}

/** Whether hls.js takes `track` for the text track of its `rendition`. */
export function namesTrack(rendition: EngineTrack, track: NamedTrack): boolean {
	void rendition;
	void track;
	return false;
}

/** The engine's id for `track`, or -1 when it is none of them. */
export function engineIdOf(track: NamedTrack, renditions: readonly EngineTrack[]): number {
	void track;
	void renditions;
	return -1;
}

/** What a pick asks of the engine: one of its renditions by id, -1 for
 *  a track that is none of them, or Off. */
export type EngineWant = number | 'off';

/** How the engine is set for a pick, and the rendition left hidden. */
export interface EnginePlan {
	subtitleTrack: number;
	subtitleDisplay: boolean;
	hidden: number | null;
}

/** The engine's setting for `want`, while the rendition `inFlight` has a
 *  fragment on the way. */
export function enginePlan(want: EngineWant, inFlight: number | null): EnginePlan {
	void want;
	void inFlight;
	return { subtitleTrack: -1, subtitleDisplay: true, hidden: null };
}

/** What the captions menu strips from a label. */
export const LABEL_MARK = '⁠';

/** `label`, for a sidecar track in `language`, kept apart from the
 *  engine's renditions. */
export function distinctLabel(
	label: string,
	language: string,
	renditions: readonly EngineTrack[]
): string {
	void language;
	void renditions;
	return label;
}

/** A text track as the sync reads and sets it. */
export interface SyncTrack extends NamedTrack {
	readonly kind: string;
	mode: CaptionMode;
}

/** A TextTrackList as the sync reads it. */
export interface SyncTrackList {
	readonly length: number;
	[index: number]: SyncTrack;
}

/** As much of the video as the sync reads. */
export interface CaptionVideo {
	readonly textTracks: SyncTrackList;
	querySelectorAll(selectors: 'track'): ArrayLike<{ readonly track: SyncTrack }>;
}

/** The engine events the sync listens to. */
export type CaptionSyncEvents = Pick<
	typeof Hls.Events,
	| 'MANIFEST_LOADING'
	| 'MANIFEST_PARSED'
	| 'FRAG_LOADING'
	| 'SUBTITLE_FRAG_PROCESSED'
	| 'SUBTITLE_TRACK_SWITCH'
	| 'ERROR'
>;

export interface CaptionSync {
	pick(target: CaptionTarget): void;
	pickTrack(track: SyncTrack | 'off'): void;
	beforeOwnTracks(start: 'own' | 'off'): void;
	release(): void;
	whenReady(fn: () => void): () => void;
	renditions(): readonly EngineTrack[];
	dispose(): void;
}

/** For now, picks set the tracks alone and the engine is not steered. */
export function createCaptionSync(
	video: CaptionVideo,
	engine: Hls,
	events: CaptionSyncEvents
): CaptionSync {
	void engine;
	void events;
	return {
		pick(target) {
			applyCaptionPick(video.textTracks, target);
		},
		pickTrack() {},
		beforeOwnTracks() {},
		release() {},
		whenReady(fn) {
			fn();
			return () => {};
		},
		renditions() {
			return [];
		},
		dispose() {}
	};
}

/** Show the track at `target`, or none, through the engine where the
 *  stream has one. */
export function pickCaptions(
	video: CaptionVideo,
	target: CaptionTarget,
	sync: CaptionSync | null
): void {
	if (sync) sync.pick(target);
	else applyCaptionPick(video.textTracks, target);
}
