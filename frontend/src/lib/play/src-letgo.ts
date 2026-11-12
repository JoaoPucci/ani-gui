/**
 * A stream the element plays from its src, let go of at a handoff
 * (unloaded, so the browser stops buffering it for nobody), and the
 * playhead it was at: the viewer's return resumes there, wherever it
 * falls. Part of the handoff gate (`$lib/play/handoff-gate`), which
 * delegates to it.
 */
export class SrcLetGo {
	private url: string | null = null;
	private at: number | null = null;

	/** Let go of `url` at `playhead`; a playhead at zero (before the
	 *  first frame) gives nothing back. */
	letGo(url: string, playhead: number): void {
		this.url = url;
		this.at = playhead > 0 ? playhead : null;
	}

	/** Whether an attach of `url` waits for the viewer: it is the stream
	 *  let go of. Another stream ends the wait and forgets the playhead. */
	attachWaits(url: string): boolean {
		if (this.url === null) return false;
		if (url === this.url) return true;
		this.forget();
		return false;
	}

	/** The point a Reload resumes at: the playhead let go of at, in
	 *  place of the unloaded element's zero. */
	reloadPoint(current: number): number {
		return this.at ?? current;
	}

	/** The playhead to resume at on the way back, once; and the wait
	 *  ends. */
	take(): number | null {
		const point = this.url !== null ? this.at : null;
		this.forget();
		return point;
	}

	private forget(): void {
		this.url = null;
		this.at = null;
	}
}
