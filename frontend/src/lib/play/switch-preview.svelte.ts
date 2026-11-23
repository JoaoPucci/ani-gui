/**
 * What "Switch to it now"'s step says about the show picked: where the
 * backend opens it ($lib/play/switch-show, `commands/switch_episode.rs`),
 * read once per pick as the step shows it.
 *
 * The latest pick's answer is the one held: an answer for an earlier
 * pick that lands later is dropped. A read that fails says nothing: the
 * switch asks again before it plays, and fails rather than take the
 * pick back.
 */

export type SwitchOwn = { episode: string } | null | 'pending';

export class SwitchPreview {
	#ask: (pickId: string) => Promise<number | null>;
	#asked = $state<string | null>(null);
	#answer = $state<{ pick: string; own: { episode: string } | null } | null>(null);

	constructor(ask: (pickId: string) => Promise<number | null>) {
		this.#ask = ask;
	}

	/** Read where a switch to `pickId` opens it, unless already asked. */
	ask(pickId: string): void {
		if (this.#asked === pickId) return;
		this.#asked = pickId;
		this.#answer = null;
		const settle = (own: { episode: string } | null) => {
			if (this.#asked === pickId) this.#answer = { pick: pickId, own };
		};
		this.#ask(pickId).then(
			(episode) => settle(episode === null ? null : { episode: String(episode) }),
			// A read that fails claims nothing: the switch asks again.
			() => undefined
		);
	}

	/** What the step says of `pickId`: its own episode, the card's line
	 *  (null), or nothing yet. */
	ownFor(pickId: string): SwitchOwn {
		const a = this.#answer;
		return a !== null && a.pick === pickId ? a.own : 'pending';
	}
}
