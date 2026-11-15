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
 *   the track stays hidden rather than disabled. Kept selected, the
 *   engine goes on fetching that rendition's next fragments, one as
 *   each lands, until its subtitle buffer is full; only then does it
 *   select none;
 * - a rendition whose fragment is on the way, and that the pick does
 *   not show, is left hidden, not disabled, until the fragment is in;
 *   a hidden track takes its cues and draws nothing.
 *
 * The engine's own switches — its default rendition picked once its
 * playlist is in, picked again after the manifest is loaded again —
 * are brought back to the app's pick within the same event, before any
 * `change` event can report them: nothing reads the switch as the
 * viewer's pick ({@link import('./caption-watch')}). A quality switch
 * to a level of another subtitle group is the exception: the engine
 * picks the new group's rendition in the language of its pick, and
 * where the rendition picked belongs to the group left, that one is
 * the pick from then on.
 *
 * hls.js tells its tracks apart by label and language. A sidecar track
 * labelled as a rendition would be taken for it — adopted as its text
 * track, or read as a pick of it — so a sidecar track's label is kept
 * apart from every rendition's name by an invisible word joiner
 * ({@link distinctLabel}), which the captions menu strips.
 *
 * The rules are pure and tested here; `createCaptionSync`
 * ({@link import('./caption-sync')}) wires them to one engine and the
 * page's video, through the track helpers of
 * {@link import('./caption-sync-tracks')}.
 */

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
 * it would show it. The engine starts the next fragment of what it
 * selects as each lands, so Off stays on the rendition, fetching
 * ahead, until its subtitle buffer is full and no fragment is on the
 * way; a hidden track keeps each fragment's cues meanwhile.
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

/** Whether two listings of the engine's renditions read the same: the
 *  same names and languages, in order. hls.js keeps a fragment on its
 *  way, under the same index, across a listing it finds identical
 *  (`subtitleOptionsIdentical`, which reads the renditions'
 *  attributes); a listing that is not renumbers them. */
export function sameListing(a: readonly EngineTrack[], b: readonly EngineTrack[]): boolean {
	return a.length === b.length && a.every((r, i) => r.name === b[i].name && r.lang === b[i].lang);
}
