/**
 * What the balloon does as a sequence moves: its primary action takes
 * focus again on each step change (the modal layer focused it on the
 * first), and every step after the first is announced, since the button
 * that keeps focus from step to step says nothing new by itself.
 * Rendering: the steps on screen.
 */
export class StepWatch {
	#last = 1;
	#moved = false;

	/** Step `index` (from 1) is up, saying `words`. */
	seen(index: number, words: string): { refocus: boolean; announce: string | null } {
		const refocus = index !== this.#last;
		this.#last = index;
		if (refocus) this.#moved = true;
		return { refocus, announce: this.#moved ? words : null };
	}
}
