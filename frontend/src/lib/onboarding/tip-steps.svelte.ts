/**
 * Where a tip on screen stands among its steps: the place each step
 * points at (its steps whose place is not on the page already left
 * out), which step is up, and whether the dim is gliding to it.
 * Rendering: the steps on screen.
 */
import { startSequence, stepSequence, type SeqState } from './sequence';

export type StepMove = 'moved' | 'stayed' | 'done';

/** The steps of `places` found on the page by `find`, in order, each
 *  with its step number from 1 (its words are named after it). A step
 *  whose place is not on the page is left out. */
export function foundSteps(
	places: readonly string[],
	find: (place: string) => HTMLElement | null
): { el: HTMLElement; n: number }[] {
	return places.flatMap((p, i) => {
		const el = find(p);
		return el ? [{ el, n: i + 1 }] : [];
	});
}

export class TipSteps {
	readonly places: HTMLElement[];
	/** Each place's step number from 1, as declared. */
	readonly numbers: number[];
	seq = $state<SeqState>({ index: 0, total: 1, done: false });
	/** Moving between steps: the dim's cut-out glides. */
	travelling = $state(false);

	constructor(places: HTMLElement[], numbers: number[] = places.map((_, i) => i + 1)) {
		this.places = places;
		this.numbers = numbers;
		this.seq = startSequence(places.length);
	}

	/** The declared number of the step that is up. */
	get number(): number {
		return this.numbers[this.seq.index];
	}

	/** The place the step that is up points at. */
	get place(): HTMLElement {
		return this.places[this.seq.index];
	}

	/** Whether every step's place is still on the page. */
	connected(): boolean {
		return this.places.every((p) => p.isConnected);
	}

	/** Next or Back. Next on the last step is Done; Back on the first
	 *  stays. */
	go(to: 'next' | 'back'): StepMove {
		const next = stepSequence(this.seq, to);
		if (next.done) return 'done';
		if (next.index === this.seq.index) return 'stayed';
		this.seq = next;
		this.travelling = true;
		return 'moved';
	}

	/** The cut-out has reached the step's place. */
	arrived(): void {
		this.travelling = false;
	}
}
