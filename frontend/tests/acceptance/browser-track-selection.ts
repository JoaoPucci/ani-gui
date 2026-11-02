// The browser's own subtitle-track selection, as Chromium does it, for
// scenarios that mount the play page under happy-dom — which has none.
//
// Chromium runs an automatic selection a moment after `<track>`
// elements join a video (a zero-delay timer, so after the task that
// added them). It leaves alone any track a script has set the mode of
// since the track joined, and it reads one flag on the video: whether a
// subtitle track was showing the last time a script changed a joined
// track's mode. With the flag up it turns on one more track among those
// left alone, even with a track already showing; with it down it turns
// on the `default` track, if none shows. A mode set before a track
// joins is no script change: it neither marks the track nor moves the
// flag, and removing a track does not move the flag either.
//
// Measured on the Electron this app ships, track elements on a video:
// - a start turned on after its tracks joined: a second track shows
//   beside it, the first listed or the one in the user's language;
// - the start already showing when it joined: it shows alone;
// - an episode with no start, after one whose start was turned on
//   after joining, or that the browser turned on itself: a track
//   shows anyway, the flag having outlived the tracks;
// - the same, with the earlier episode's tracks switched off before
//   they were removed: none shows.
//
// The model turns on the first track left alone where Chromium may
// prefer the user's language; scenarios assert how many tracks show,
// not which extra one. happy-dom's `track` getter also builds a new
// TextTrack on every read, so each element here keeps the first it
// builds, as a browser's does, holding any of the three modes. And
// happy-dom starts a `default` track showing; here, as in Chromium,
// every track starts disabled until a script or the selection turns
// it on — a mode set before the track joins is the script's, but not
// a change to a joined track.
//
// What the model cannot show: happy-dom builds a new TextTrackList on
// every read of `textTracks`, so the change event the browser fires
// after its selection never reaches the page's listener here. Whether
// the start joins already showing — which keeps the selection from
// changing anything the page would have to hear — is held by the
// unit tests in sidecar-tracks.dom.test.ts.

interface Installed {
	uninstall(): void;
}

export function installBrowserTrackSelection(): Installed {
	const descriptor = Object.getOwnPropertyDescriptor(HTMLTrackElement.prototype, 'track');
	const build = descriptor?.get;
	if (!build) throw new Error('happy-dom has no HTMLTrackElement.track getter to stabilise');
	const appendDescriptor = Object.getOwnPropertyDescriptor(Node.prototype, 'appendChild');
	const append = appendDescriptor?.value as Node['appendChild'];

	const owned = new WeakMap<HTMLTrackElement, TextTrack>();
	const setByScript = new WeakSet<TextTrack>();
	const flagUp = new WeakMap<Node, boolean>();
	const pending = new WeakSet<Node>();
	let browserActing = false;

	const joined = (video: Node) =>
		Array.from((video as Element).querySelectorAll('track')).map((el) => ({
			el,
			track: el.track
		}));

	function select(video: Node) {
		pending.delete(video);
		const tracks = joined(video);
		browserActing = true;
		try {
			if (flagUp.get(video)) {
				const extra = tracks.find((t) => !setByScript.has(t.track) && t.track.mode !== 'showing');
				if (extra) extra.track.mode = 'showing';
			} else if (!tracks.some((t) => t.track.mode === 'showing')) {
				const byDefault = tracks.find((t) => t.el.default);
				if (byDefault) byDefault.track.mode = 'showing';
			}
		} finally {
			browserActing = false;
		}
		flagUp.set(
			video,
			tracks.some((t) => t.track.mode === 'showing')
		);
	}

	Object.defineProperty(HTMLTrackElement.prototype, 'track', {
		configurable: true,
		get(this: HTMLTrackElement) {
			return ownTrack(this);
		}
	});

	function ownTrack(el: HTMLTrackElement): TextTrack {
		let track = owned.get(el);
		if (!track) {
			const built = build!.call(el) as TextTrack;
			let mode: TextTrackMode = 'disabled';
			Object.defineProperty(built, 'mode', {
				configurable: true,
				get: () => mode,
				set: (next: TextTrackMode) => {
					if (next !== 'showing' && next !== 'hidden' && next !== 'disabled') return;
					if (next === mode) return;
					mode = next;
					const video = el.parentNode;
					if (!browserActing && video instanceof HTMLMediaElement) {
						setByScript.add(built);
						flagUp.set(
							video,
							joined(video).some((t) => t.track.mode === 'showing')
						);
					}
				}
			});
			track = built;
			owned.set(el, track);
		}
		return track;
	}

	/** The selection runs once per batch of tracks, a task later. */
	function schedule(video: HTMLMediaElement) {
		if (pending.has(video)) return;
		pending.add(video);
		setTimeout(() => select(video), 0);
	}

	Object.defineProperty(Node.prototype, 'appendChild', {
		configurable: true,
		writable: true,
		value: function <T extends Node>(this: Node, node: T): T {
			const added = append.call(this, node) as T;
			if (node instanceof HTMLTrackElement && this instanceof HTMLMediaElement) schedule(this);
			return added;
		}
	});

	return {
		uninstall() {
			if (descriptor) Object.defineProperty(HTMLTrackElement.prototype, 'track', descriptor);
			if (appendDescriptor) Object.defineProperty(Node.prototype, 'appendChild', appendDescriptor);
		}
	};
}
