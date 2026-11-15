/**
 * The video's tracks as the captions sync ({@link import('./caption-sync')})
 * reads them: which track is whose, what a pick asks of the engine, and
 * the sync without an engine, for a stream played from the element's
 * own source.
 */

import type Hls from 'hls.js';
import {
	engineIdOf,
	namesTrack,
	type EngineTrack,
	type EngineWant,
	type NamedTrack
} from './caption-engine';
import { applyCaptionPick, type CaptionMode, type CaptionTarget } from './caption-pick';

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

/** As much of the video as the sync reads: its tracks, and its
 *  `<track>` elements, whose tracks are the app's sidecar ones. */
export interface CaptionVideo {
	readonly textTracks: SyncTrackList;
	querySelectorAll(selectors: 'track'): ArrayLike<{ readonly track: SyncTrack }>;
}

export const isCaption = (t: SyncTrack) => t.kind === 'subtitles' || t.kind === 'captions';

/** `track`'s index in `list`, or -1. */
export function indexIn(list: SyncTrackList, track: SyncTrack): number {
	for (let i = 0; i < list.length; i++) if (list[i] === track) return i;
	return -1;
}

/** The index of the engine's text track for `rendition`: the first
 *  caption track it names that is not a sidecar track, or -1. */
export function renditionIndex(
	list: SyncTrackList,
	rendition: EngineTrack | undefined,
	sidecars: ReadonlySet<SyncTrack>
): number {
	if (!rendition) return -1;
	for (let i = 0; i < list.length; i++) {
		const t = list[i];
		if (isCaption(t) && !sidecars.has(t) && namesTrack(rendition, t)) return i;
	}
	return -1;
}

/** What the sync shows: a track on the video, one of the app's own
 *  about to join it ('own'), or Off. */
export type Desired = SyncTrack | 'own' | 'off';

/** What `desired`, at `at` in the list (-1 for not there), asks of the
 *  engine. */
export function wantOf(
	desired: Desired,
	at: number,
	sidecars: ReadonlySet<SyncTrack>,
	renditions: readonly EngineTrack[]
): EngineWant {
	if (desired === 'own') return -1;
	if (desired === 'off' || at === -1) return 'off';
	return sidecars.has(desired) ? -1 : engineIdOf(desired, renditions);
}

/** The engine events the sync listens to, as the page's engine names
 *  them. */
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
	/** Show the track at `target` in the video's TextTrackList, or
	 *  none: a pick from either captions menu. */
	pick(target: CaptionTarget): void;
	/** Show `track`, or none. */
	pickTrack(track: SyncTrack | 'off'): void;
	/** Set the engine aside for tracks of the app's own about to join
	 *  the video, one of which ('own') or none ('off') will show, and
	 *  stop it picking its playlist's default once its group arrives. */
	beforeOwnTracks(start: 'own' | 'off'): void;
	/** Stop steering the engine: the app's tracks are leaving. */
	release(): void;
	/** Run `fn` once the engine's playlist names its renditions — at
	 *  once if it does already. The returned function cancels it. */
	whenReady(fn: () => void): () => void;
	/** Every rendition the playlist lists, in any group. */
	renditions(): readonly EngineTrack[];
	/** The engine retired. */
	dispose(): void;
}

/** The captions without an engine: picks set the tracks alone, tracks
 *  of the app's own join with every track off, and nothing waits. */
export function plainCaptions(video: CaptionVideo): CaptionSync {
	return {
		pick: (target) => applyCaptionPick(video.textTracks, target),
		pickTrack: () => {},
		beforeOwnTracks: () => applyCaptionPick(video.textTracks, 'off'),
		release: () => {},
		whenReady: (fn) => {
			fn();
			return () => {};
		},
		renditions: () => [],
		dispose: () => {}
	};
}

/** Show the track at `target`, or none, through the engine where the
 *  stream has one. */
export function pickCaptions(
	video: CaptionVideo,
	target: CaptionTarget,
	sync: CaptionSync | null
): void {
	(sync ?? plainCaptions(video)).pick(target);
}
