/**
 * hls.js's own subtitle selection, kept in step with the captions the
 * app shows, on a stream whose playlist carries subtitle renditions —
 * alone, or beside the sidecar tracks the app attaches
 * ({@link import('./sidecar-tracks')}).
 *
 * hls.js reads and writes the video's text tracks itself. Each time
 * its subtitle track is set it disables every labelled subtitle track
 * but the one it chose — the app's sidecar tracks included — and a
 * moment after any change to the tracks it reads which one shows and
 * takes it for a pick of its own: one of its renditions, or none. A
 * sidecar track shown while hls.js had a rendition read as "none of
 * mine", and hls.js switched every track off. And it drops the cues of
 * a rendition's fragment that it parses while that rendition's track
 * is disabled, without ever fetching the fragment again: a rendition
 * switched off while its fragment was on the way showed nothing for
 * the rest of the play.
 *
 * So every captions pick — the app's menu, Chromium's own menu with
 * the custom controls off, the session's start — is made through the
 * engine first, and the tracks set after it:
 *
 * - a rendition: the engine selects it and displays it;
 * - a sidecar track, or a track none of the engine's: the engine
 *   selects none, so it reads the shown track as agreeing with it;
 * - Off: the engine selects none — or, while one of its fragments is
 *   on the way, keeps that rendition selected with its display off, so
 *   the track stays hidden rather than disabled;
 * - a rendition whose fragment is on the way, and that the pick does
 *   not show, is left hidden, not disabled, until the fragment is in;
 *   a hidden track takes its cues and draws nothing.
 *
 * The engine's own switches — its default rendition picked once its
 * playlist is in, picked again after the manifest is loaded again or
 * the subtitle group changes with the quality — are brought back to
 * the app's pick within the same event, before any `change` event can
 * report them: nothing reads the switch as the viewer's pick
 * ({@link import('./caption-watch')}).
 *
 * hls.js tells its tracks apart by label and language. A sidecar track
 * labelled as a rendition would be taken for it — adopted as its text
 * track, or read as a pick of it — so a sidecar track's label is kept
 * apart from every rendition's name by an invisible word joiner
 * ({@link distinctLabel}), which the captions menu strips.
 *
 * The rules are pure and tested here; {@link createCaptionSync} wires
 * them to one engine and the page's video.
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

/** Whether hls.js takes `track` for the text track of its `rendition`:
 *  the same label, ignoring case, and the same language where the
 *  track has one (hls.js's `subtitleTrackMatchesTextTrack`). */
export function namesTrack(rendition: EngineTrack, track: NamedTrack): boolean {
	return (
		track.label.toLowerCase() === rendition.name.toLowerCase() &&
		(!track.language || track.language.toLowerCase() === (rendition.lang ?? '').toLowerCase())
	);
}

/** The engine's id for `track` — its index among the engine's
 *  renditions — or -1 when it is none of them. */
export function engineIdOf(track: NamedTrack, renditions: readonly EngineTrack[]): number {
	return renditions.findIndex((r) => namesTrack(r, track));
}

/** What a pick asks of the engine: one of its renditions by id, -1 for
 *  a track that is none of them, or Off. */
export type EngineWant = number | 'off';

/** How the engine is set for a pick, and the rendition left hidden. */
export interface EnginePlan {
	/** The engine's subtitle track: a rendition's id, or -1 for none. */
	subtitleTrack: number;
	/** Whether the engine shows its subtitle track or keeps it hidden. */
	subtitleDisplay: boolean;
	/** The rendition whose text track is left hidden, not disabled, so
	 *  the fragment on its way keeps its cues; null for none. */
	hidden: number | null;
}

/**
 * The engine's setting for `want`, while the rendition `inFlight` has a
 * fragment on the way (null for none).
 *
 * Off with a fragment on the way keeps that rendition selected with
 * its display off: a rendition left hidden while the engine selects
 * none, with nothing showing, is read by the engine as its pick, and
 * it would show it.
 */
export function enginePlan(want: EngineWant, inFlight: number | null): EnginePlan {
	if (want === 'off') {
		return inFlight === null
			? { subtitleTrack: -1, subtitleDisplay: true, hidden: null }
			: { subtitleTrack: inFlight, subtitleDisplay: false, hidden: inFlight };
	}
	const subtitleTrack = want < 0 ? -1 : want;
	return {
		subtitleTrack,
		subtitleDisplay: true,
		hidden: inFlight !== null && inFlight !== subtitleTrack ? inFlight : null
	};
}

/** What the captions menu strips from a label: the mark that keeps a
 *  sidecar track's label apart from a rendition's name. */
export const LABEL_MARK = '\u2060';

/** `label`, for a sidecar track in `language`, with as many marks as
 *  keep the engine from taking it for any of its renditions. */
export function distinctLabel(
	label: string,
	language: string,
	renditions: readonly EngineTrack[]
): string {
	let out = label;
	while (renditions.some((r) => namesTrack(r, { label: out, language }))) out += LABEL_MARK;
	return out;
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

/** As much of the video as the sync reads: its tracks, and its
 *  `<track>` elements, whose tracks are the app's sidecar ones. */
export interface CaptionVideo {
	readonly textTracks: SyncTrackList;
	querySelectorAll(selectors: 'track'): ArrayLike<{ readonly track: SyncTrack }>;
}

const isCaption = (t: SyncTrack) => t.kind === 'subtitles' || t.kind === 'captions';

/** `track`'s index in `list`, or -1. */
function indexIn(list: SyncTrackList, track: SyncTrack): number {
	for (let i = 0; i < list.length; i++) if (list[i] === track) return i;
	return -1;
}

/** The index of the engine's text track for `rendition`: the first
 *  caption track it names that is not a sidecar track, or -1. */
function renditionIndex(
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
type Desired = SyncTrack | 'own' | 'off';

/** What `desired`, at `at` in the list (-1 for not there), asks of the
 *  engine. */
function wantOf(
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

/** Keep `engine`'s subtitle selection in step with the captions shown
 *  on `video`. */
export function createCaptionSync(
	video: CaptionVideo,
	engine: Hls,
	events: CaptionSyncEvents
): CaptionSync {
	let desired: Desired | null = null;
	// The subtitle fragment on its way, if any: the engine loads one at
	// a time, and starts the next as it finishes one, before this hears
	// that the one before is in.
	let inFlight: { level: number; sn: unknown } | null = null;
	let ready = false;
	let applying = false;
	let disposed = false;
	let waiting: (() => void)[] = [];

	/** `force`: set the engine's subtitle track even where it is the
	 *  plan's already, which also turns off the engine's own pick of its
	 *  playlist's default once the playlist's group arrives. */
	const apply = (force = false): void => {
		if (disposed || desired === null) return;
		const list = video.textTracks;
		const sidecars = new Set(Array.from(video.querySelectorAll('track'), (el) => el.track));
		const renditions = engine.subtitleTracks;
		const at = typeof desired === 'string' ? -1 : indexIn(list, desired);
		const plan = enginePlan(wantOf(desired, at, sidecars, renditions), inFlight?.level ?? null);
		applying = true;
		try {
			if (force || engine.subtitleTrack !== plan.subtitleTrack)
				engine.subtitleTrack = plan.subtitleTrack;
			if (engine.subtitleDisplay !== plan.subtitleDisplay)
				engine.subtitleDisplay = plan.subtitleDisplay;
		} finally {
			applying = false;
		}
		const hidden =
			plan.hidden === null ? -1 : renditionIndex(list, renditions[plan.hidden], sidecars);
		applyCaptionPick(list, at === -1 ? 'off' : at, hidden === -1 ? undefined : hidden);
	};

	const show = (next: Desired, force = false): void => {
		desired = next;
		apply(force);
	};

	const isSubtitle = (frag: { type: unknown } | undefined) =>
		frag !== undefined && `${frag.type}` === 'subtitle';

	engine.on(events.MANIFEST_LOADING, () => {
		ready = false;
		inFlight = null;
	});
	engine.on(events.MANIFEST_PARSED, () => {
		ready = true;
		const run = waiting;
		waiting = [];
		for (const fn of run) fn();
	});
	/** The fragment on its way is in, or failed: its rendition need no
	 *  longer stay hidden. */
	const landed = (frag: { level: number; sn: unknown }): void => {
		if (!inFlight || frag.level !== inFlight.level || frag.sn !== inFlight.sn) return;
		inFlight = null;
		apply();
	};
	engine.on(events.FRAG_LOADING, (_event, data) => {
		if (isSubtitle(data.frag)) inFlight = { level: data.frag.level, sn: data.frag.sn };
	});
	engine.on(events.SUBTITLE_FRAG_PROCESSED, (_event, data) => landed(data.frag));
	engine.on(events.ERROR, (_event, data) => {
		if (data.frag && isSubtitle(data.frag)) landed(data.frag);
	});
	// The engine's own switch is brought back to the app's pick inside
	// the event, so no `change` event ever reports it.
	engine.on(events.SUBTITLE_TRACK_SWITCH, () => {
		if (!applying) apply();
	});

	return {
		pick(target) {
			show(target === 'off' ? 'off' : (video.textTracks[target] ?? 'off'));
		},
		pickTrack(track) {
			show(track);
		},
		// The engine's own default pick, made later, would switch the
		// tracks against the start before it could be brought back.
		beforeOwnTracks(start) {
			show(start, true);
		},
		release() {
			desired = null;
		},
		whenReady(fn) {
			if (ready) {
				fn();
				return () => {};
			}
			waiting.push(fn);
			return () => {
				waiting = waiting.filter((w) => w !== fn);
			};
		},
		renditions() {
			return engine.allSubtitleTracks;
		},
		dispose() {
			disposed = true;
			desired = null;
			waiting = [];
		}
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
