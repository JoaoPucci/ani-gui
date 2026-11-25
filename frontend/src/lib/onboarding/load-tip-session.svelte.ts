/**
 * The player's load tip on one player visit ($./load-tip decides; this
 * keeps its clock and its side effects). Rendering glue, kept out of
 * the component so it can be tested.
 *
 * - It is fed the start veil and what blocks the tip whenever they
 *   change, and keeps a timer so a start that lasts SHOW_AFTER_MS shows
 *   it with no further change to feed.
 * - It tells a source's first load from a later start: once a start of
 *   the source has ended, the source has played, until `newSource`.
 * - Closed by the person, it says so once (`onSeen`); gone by any
 *   route, it says whether focus was in it (`onGone`), so focus can
 *   move to the player rather than drop to the page.
 */
import {
	LOAD_TIP_IDLE,
	SHOW_AFTER_MS,
	loadTipClose,
	loadTipStep,
	loadTipUp,
	type LoadTipInput,
	type LoadTipState
} from './load-tip';

export interface LoadTipDeps {
	now: () => number;
	setTimer: (fn: () => void, ms: number) => ReturnType<typeof setTimeout>;
	clearTimer: (t: ReturnType<typeof setTimeout>) => void;
	/** The person closed it. */
	onSeen: () => void;
	/** It left the screen, by any route. */
	onGone: () => void;
}

export class LoadTipSession {
	state = $state<LoadTipState>(LOAD_TIP_IDLE);
	readonly #deps: LoadTipDeps;
	#last: Omit<LoadTipInput, 'now' | 'fresh'> = { veil: 'off', blocked: false };
	/** This source has been seen starting, and then playing. */
	#sawStart = false;
	#played = false;
	#timer: ReturnType<typeof setTimeout> | null = null;

	constructor(deps: LoadTipDeps) {
		this.#deps = deps;
	}

	/** Whether it is on screen. */
	get up(): boolean {
		return loadTipUp(this.state);
	}

	/** The veil, or what blocks the tip, changed. A start that ends
	 *  marks the source played: a start after that is mid-playback. */
	feed(input: Omit<LoadTipInput, 'now' | 'fresh'>): void {
		this.#last = input;
		const fresh = !this.#played;
		this.observe(input.veil);
		this.#step(loadTipStep(this.state, { ...input, fresh, now: this.#deps.now() }));
		this.#arm();
	}

	/** Got it, or Escape. */
	close(): void {
		this.#step(loadTipClose(this.state));
	}

	/** The veil changed, whether or not a tip is chosen: a start that
	 *  ends marks the source played. */
	observe(veil: LoadTipInput['veil']): void {
		if (veil === 'start') this.#sawStart = true;
		else if (veil === 'off' && this.#sawStart) this.#played = true;
	}

	/** Another episode's stream: a wait under way starts afresh, a tip
	 *  that went unseen waits for its load, one seen stays gone. */
	newSource(): void {
		this.#sawStart = false;
		this.#played = false;
		const unseenGone = this.state.phase === 'gone' && !this.state.seen;
		if (unseenGone || this.state.phase === 'waiting') {
			this.#clear();
			this.state = LOAD_TIP_IDLE;
		}
		// Its start may be up already: look again.
		this.feed(this.#last);
	}

	stop(): void {
		this.#clear();
	}

	#step(next: LoadTipState): void {
		const was = this.state;
		if (next === was) return;
		this.state = next;
		this.#clear();
		if (next.phase === 'gone' && loadTipUp(was)) {
			if (next.seen) this.#deps.onSeen();
			this.#deps.onGone();
		}
	}

	/** While a start is waited out, look again once it has lasted long
	 *  enough: a timer that fires a hair early looks again too. */
	#arm(): void {
		if (this.state.phase !== 'waiting' || this.#timer !== null) return;
		const waited = this.#deps.now() - (this.state.startedAt ?? 0);
		this.#timer = this.#deps.setTimer(
			() => {
				this.#timer = null;
				this.feed(this.#last);
			},
			Math.max(1, SHOW_AFTER_MS - waited)
		);
	}

	#clear(): void {
		if (this.#timer !== null) this.#deps.clearTimer(this.#timer);
		this.#timer = null;
	}
}
