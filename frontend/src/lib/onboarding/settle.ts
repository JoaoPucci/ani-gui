/**
 * The settle timers behind the place watcher ($./watch): a place
 * settles once it has been on screen, unmoved, for SETTLE_MS. Each
 * sighting restarts its timer; a sighting off screen, or a drop, takes
 * it out. Rendering: the screen's places, the screen's clock.
 */
import { SETTLE_MS } from './gate';

export interface Timers {
	setTimer: (fn: () => void, ms: number) => ReturnType<typeof setTimeout>;
	clearTimer: (t: ReturnType<typeof setTimeout>) => void;
}

export class Settler {
	readonly #timers = new Map<string, ReturnType<typeof setTimeout>>();
	readonly #settled = new Set<string>();
	readonly #clock: Timers;
	readonly #onChange: (settled: ReadonlySet<string>) => void;

	constructor(onChange: (settled: ReadonlySet<string>) => void, clock: Timers) {
		this.#onChange = onChange;
		this.#clock = clock;
	}

	/** The place was seen, moved, on screen or not: it settles SETTLE_MS
	 *  from now if it was on screen and nothing moves it again. */
	sighted(place: string, onScreen: boolean): void {
		this.drop(place);
		if (!onScreen) return;
		this.#timers.set(
			place,
			this.#clock.setTimer(() => {
				this.#timers.delete(place);
				this.#settled.add(place);
				this.#onChange(this.#settled);
			}, SETTLE_MS)
		);
	}

	/** The place is not settled, and is not settling. */
	drop(place: string): void {
		const t = this.#timers.get(place);
		if (t !== undefined) this.#clock.clearTimer(t);
		this.#timers.delete(place);
		if (this.#settled.delete(place)) this.#onChange(this.#settled);
	}

	/** Stop every timer. */
	stop(): void {
		for (const t of this.#timers.values()) this.#clock.clearTimer(t);
		this.#timers.clear();
	}
}
