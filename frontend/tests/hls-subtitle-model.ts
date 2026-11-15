// hls.js's subtitle handling, as hls.js 1.6.19 does it, for tests that
// drive the app's captions against a stream whose playlist carries
// subtitle renditions. Neither happy-dom nor Node runs hls.js (it needs
// Media Source Extensions); the packaged-app e2e tier runs the real one.
//
// Read from node_modules/hls.js/src:
// - SubtitleTrackController.setSubtitleTrack sets its pick and runs
//   toggleTrackModes — every labelled subtitles/captions text track but
//   the one matching its pick (by label, ignoring case, and language)
//   disabled, that one showing or hidden as subtitleDisplay says — then
//   triggers SUBTITLE_TRACK_SWITCH, all synchronously. The
//   `subtitleTrack` setter also turns its default selection off.
// - It listens to the video's TextTrackList `change` and, from a
//   zero-delay timer, takes the first showing track (else the last
//   hidden one) for its pick, switching when that differs.
// - A manifest load (MANIFEST_LOADING) resets its pick and turns its
//   default selection back on; when the level's subtitle group arrives
//   (switchLevel) it lists the group (SUBTITLE_TRACKS_UPDATED) and, with
//   the default selection on, picks the rendition marked DEFAULT.
// - On a quality switch to a level of another subtitle group,
//   switchLevel drops its pick, lists the new group, and picks again
//   the new group's rendition in its pick's language
//   (findTrackId's LANGUAGE match).
// - A seek aborts the fragment on its way with no event: its loader's
//   stats read `aborted`.
// - TimelineController makes one text track per rendition on
//   SUBTITLE_TRACKS_UPDATED, created disabled — or takes over a track
//   already on the video with the same kind, label and language — and
//   drops the cues of a fragment it parses while that track is
//   disabled; the fragment is not fetched again.
//
// Where the model is simpler than hls.js, and no test here relies on
// the difference: setSubtitleTrack always triggers SUBTITLE_TRACK_SWITCH
// (hls.js skips it for the track already picked and loaded, after
// still setting the modes); the read of the captions runs once per
// `change` (hls.js coalesces reads into one timer); and a track taken
// over is not taken off the list of tracks in use, so two renditions
// named alike would share one.

export interface ModelTrack {
	readonly kind: string;
	readonly label: string;
	readonly language: string;
	mode: 'showing' | 'hidden' | 'disabled';
}

export interface ModelTrackList extends EventTarget {
	readonly length: number;
	[index: number]: ModelTrack;
}

export interface ModelVideo {
	readonly textTracks: ModelTrackList;
}

export interface Rendition {
	name: string;
	lang: string;
	default?: boolean;
}

type Handler = (event: string, data: unknown) => void;

export const MODEL_EVENTS = {
	MANIFEST_LOADING: 'hlsManifestLoading',
	MANIFEST_PARSED: 'hlsManifestParsed',
	FRAG_LOADING: 'hlsFragLoading',
	FRAG_LOADED: 'hlsFragLoaded',
	SUBTITLE_FRAG_PROCESSED: 'hlsSubtitleFragProcessed',
	SUBTITLE_TRACK_SWITCH: 'hlsSubtitleTrackSwitch',
	SUBTITLE_TRACKS_UPDATED: 'hlsSubtitleTracksUpdated',
	ERROR: 'hlsError',
	BACK_BUFFER_REACHED: 'hlsBackBufferReached',
	LEVELS_UPDATED: 'hlsLevelsUpdated',
	FRAG_BUFFERED: 'hlsFragBuffered',
	FRAG_CHANGED: 'hlsFragChanged',
	FRAG_PARSING_INIT_SEGMENT: 'hlsFragParsingInitSegment'
} as const;

const isCaption = (t: ModelTrack) =>
	(t.kind === 'subtitles' || t.kind === 'captions') && t.label !== '';
const names = (r: Rendition, t: ModelTrack) =>
	t.label.toLowerCase() === r.name.toLowerCase() &&
	(!t.language || t.language.toLowerCase() === r.lang.toLowerCase());

export class HlsSubtitleModel {
	handlers: Record<string, Handler[]> = {};
	subtitleTracks: Rendition[] = [];
	allSubtitleTracks: Rendition[] = [];
	/** Each rendition's text track, by id. */
	textTracks: ModelTrack[] = [];
	/** The cues each rendition's track holds, by id. */
	cues: string[][] = [];
	/** Every pick the engine made of its own accord. */
	ownSwitches: number[] = [];
	private trackId = -1;
	private display = true;
	private selectDefault = true;
	private video: ModelVideo | null = null;
	private sn = 0;

	constructor(
		/** Makes a text track on the video, as `addTextTrack` does. */
		private makeTrack: (kind: string, label: string, lang: string) => ModelTrack
	) {}

	on(event: string, handler: Handler) {
		(this.handlers[event] ??= []).push(handler);
	}
	off(event: string, handler: Handler) {
		this.handlers[event] = (this.handlers[event] ?? []).filter((h) => h !== handler);
	}
	emit(event: string, data: unknown = {}) {
		for (const h of [...(this.handlers[event] ?? [])]) h(event, data);
	}

	attach(video: ModelVideo) {
		this.video = video;
		video.textTracks.addEventListener('change', () => {
			setTimeout(() => this.poll(), 0);
		});
	}

	get subtitleTrack(): number {
		return this.trackId;
	}
	set subtitleTrack(id: number) {
		this.selectDefault = false;
		this.setSubtitleTrack(id);
	}
	get subtitleDisplay(): boolean {
		return this.display;
	}
	set subtitleDisplay(value: boolean) {
		this.display = value;
		if (this.trackId > -1) this.toggle();
	}

	private setSubtitleTrack(id: number) {
		if (id < -1 || id >= this.subtitleTracks.length) return;
		this.selectDefault = false;
		this.trackId = id;
		this.toggle();
		this.emit(MODEL_EVENTS.SUBTITLE_TRACK_SWITCH, { id });
	}

	private labelled(): ModelTrack[] {
		const list = this.video?.textTracks;
		const out: ModelTrack[] = [];
		if (list) for (let i = 0; i < list.length; i++) if (isCaption(list[i])) out.push(list[i]);
		return out;
	}

	private toggle() {
		const current = this.subtitleTracks[this.trackId];
		const tracks = this.labelled();
		const next = current ? tracks.find((t) => names(current, t)) : undefined;
		for (const t of tracks) if (t !== next && t.mode !== 'disabled') t.mode = 'disabled';
		const mode = this.display ? 'showing' : 'hidden';
		if (next && next.mode !== mode) next.mode = mode;
	}

	private poll() {
		let shown: ModelTrack | null = null;
		for (const t of this.labelled()) {
			if (t.mode === 'hidden') shown = t;
			else if (t.mode === 'showing') {
				shown = t;
				break;
			}
		}
		const id = shown ? this.subtitleTracks.findIndex((r) => names(r, shown)) : -1;
		if (id !== this.trackId) {
			this.ownSwitches.push(id);
			this.setSubtitleTrack(id);
		}
	}

	/** A manifest load begins: the pick is reset, the default selection
	 *  turned back on. */
	manifestLoading() {
		this.trackId = -1;
		this.selectDefault = true;
		this.emit(MODEL_EVENTS.MANIFEST_LOADING, {});
	}

	/** The manifest is parsed: its renditions are known. */
	manifestParsed(renditions: Rendition[]) {
		this.allSubtitleTracks = renditions;
		this.emit(MODEL_EVENTS.MANIFEST_PARSED, { levels: [], subtitleTracks: renditions });
	}

	/** The level's subtitle group arrives: one text track per rendition,
	 *  and the default picked while the default selection is on. */
	groupArrives() {
		this.subtitleTracks = this.allSubtitleTracks;
		const inUse = this.labelled();
		this.textTracks = this.subtitleTracks.map((r) => {
			const reused = inUse.find((t) => t.kind === 'subtitles' && names(r, t));
			if (reused) return reused;
			const made = this.makeTrack('subtitles', r.name, r.lang);
			made.mode = 'disabled';
			return made;
		});
		this.cues = this.subtitleTracks.map(() => []);
		this.emit(MODEL_EVENTS.SUBTITLE_TRACKS_UPDATED, { subtitleTracks: this.subtitleTracks });
		const at = this.subtitleTracks.findIndex((r) => r.default === true);
		if (this.selectDefault && at !== -1 && this.trackId === -1) {
			this.ownSwitches.push(at);
			this.setSubtitleTrack(at);
		}
	}

	/** The fragments started, by number, as the engine holds them. */
	frags: { type: string; level: number; sn: number; stats: { aborted: boolean } }[] = [];

	/** A fragment of rendition `level` starts loading; its number. */
	fragmentLoading(level: number): number {
		const sn = this.sn++;
		const frag = { type: 'subtitle', level, sn, stats: { aborted: false } };
		this.frags[sn] = frag;
		this.emit(MODEL_EVENTS.FRAG_LOADING, { frag });
		return sn;
	}

	/** A seek aborts the fragment on its way: no event, its stats say so. */
	fragmentAborted(sn: number) {
		this.frags[sn].stats.aborted = true;
	}

	/** A quality switch to a level of another subtitle group: the pick
	 *  dropped, the group listed, the pick's language picked again. */
	groupChanges(renditions: Rendition[]) {
		const lang = this.subtitleTracks[this.trackId]?.lang;
		this.trackId = -1;
		this.allSubtitleTracks = [...this.allSubtitleTracks, ...renditions];
		this.subtitleTracks = renditions;
		const inUse = this.labelled();
		this.textTracks = renditions.map((r) => {
			const reused = inUse.find((t) => t.kind === 'subtitles' && names(r, t));
			if (reused) return reused;
			const made = this.makeTrack('subtitles', r.name, r.lang);
			made.mode = 'disabled';
			return made;
		});
		this.cues = renditions.map(() => []);
		this.emit(MODEL_EVENTS.SUBTITLE_TRACKS_UPDATED, { subtitleTracks: renditions });
		const at = lang === undefined ? -1 : renditions.findIndex((r) => r.lang === lang);
		if (at !== -1) {
			this.ownSwitches.push(at);
			this.setSubtitleTrack(at);
		}
	}

	/** The fragment is in and parsed: its cues go into the rendition's
	 *  track, unless that track is disabled, when they are dropped. */
	fragmentLands(level: number, sn: number, cue: string) {
		const track = this.textTracks[level];
		if (track && track.mode !== 'disabled') this.cues[level].push(cue);
		this.emit(MODEL_EVENTS.SUBTITLE_FRAG_PROCESSED, {
			success: true,
			frag: { type: 'subtitle', level, sn }
		});
	}
}

/** A video for unit tests: its `<track>` elements' tracks first, in the
 *  order they joined, then the ones made with `addTextTrack`, as
 *  Chromium lists them; `change` on the list a task after any mode
 *  changes, once however many did. */
export class ModelTrackVideo {
	readonly elements: { track: ModelTrack }[] = [];
	readonly made: ModelTrack[] = [];
	readonly textTracks: ModelTrackList;
	private pending = false;

	constructor() {
		const events = new EventTarget();
		const all = () => [...this.elements.map((e) => e.track), ...this.made];
		this.textTracks = new Proxy(events, {
			get: (target, prop) => {
				if (prop === 'length') return all().length;
				if (typeof prop === 'string' && /^\d+$/.test(prop)) return all()[Number(prop)];
				const value = Reflect.get(target, prop, target);
				return typeof value === 'function' ? value.bind(target) : value;
			}
		}) as unknown as ModelTrackList;
	}

	private track(kind: string, label: string, language: string): ModelTrack {
		let mode: ModelTrack['mode'] = 'disabled';
		const changed = () => this.schedule();
		return {
			kind,
			label,
			language,
			get mode() {
				return mode;
			},
			set mode(next) {
				if (next === mode) return;
				mode = next;
				changed();
			}
		};
	}

	schedule() {
		if (this.pending) return;
		this.pending = true;
		setTimeout(() => {
			this.pending = false;
			this.textTracks.dispatchEvent(new Event('change'));
		}, 0);
	}

	/** A track the engine makes, as `addTextTrack` does. */
	addTextTrack = (kind: string, label: string, language: string): ModelTrack => {
		const t = this.track(kind, label, language);
		this.made.push(t);
		return t;
	};

	/** A `<track>` element's track joining the video. */
	addElement(label: string, language: string, mode: ModelTrack['mode'] = 'disabled'): ModelTrack {
		const t = this.track('subtitles', label, language);
		// A mode set before the track joins is no change to a joined one.
		const element = { track: t };
		this.elements.push(element);
		if (mode !== 'disabled') t.mode = mode;
		return t;
	}

	querySelectorAll(selectors: 'track'): ArrayLike<{ readonly track: ModelTrack }> {
		void selectors;
		return this.elements;
	}
}
