/**
 * The balloon as the page's topmost modal ($lib/components/modal-layer):
 * focus moves to its primary action, Tab stays in it, the app's own
 * shortcuts stand down, and the rest of the page is inert except what a
 * press must still reach: the step's place, the window's own buttons
 * and the dim's catcher. Rendering glue for the spotlight session.
 */
import { isTopModal, openModal, type ModalHandle } from '$lib/components/modal-layer';
import { WINDOW_BUTTONS } from './screen-read';

export class BalloonHold {
	readonly #doc: Document;
	readonly #onEscape: () => void;
	#modal: ModalHandle | null = null;
	#root: HTMLElement | null = null;
	#scrim: HTMLElement | null = null;

	constructor(doc: Document, onEscape: () => void) {
		this.#doc = doc;
		this.#onEscape = onEscape;
	}

	/** The balloon, once it holds the page. */
	get root(): HTMLElement | null {
		return this.#root;
	}

	/** Whether the balloon holds the page (it may since be covered). */
	get held(): boolean {
		return this.#modal !== null;
	}

	/** Take the page, keeping `place` in reach. Once. */
	take(root: HTMLElement, scrim: HTMLElement | null, place: HTMLElement): void {
		if (this.#modal) return;
		this.#root = root;
		this.#scrim = scrim;
		this.#modal = openModal(root, { onEscape: this.#onEscape, live: this.#live(place) });
	}

	/** The step moved: keep its new place in reach instead. */
	follow(place: HTMLElement): void {
		this.#modal?.update({ onEscape: this.#onEscape, live: this.#live(place) });
	}

	/** Held, with another modal opened over it. */
	covered(): boolean {
		return this.#root !== null && this.#modal !== null && !isTopModal(this.#root);
	}

	/** Give the page back. Says whether a modal on top kept it (or it was
	 *  never held), in which case focus is not the tip's to move. */
	release(): { covered: boolean } {
		const covered = this.#root === null || !isTopModal(this.#root);
		this.#modal?.close();
		this.#modal = null;
		return { covered };
	}

	#live(place: HTMLElement): Element[] {
		const live: Element[] = [place, ...this.#doc.querySelectorAll(WINDOW_BUTTONS)];
		if (this.#scrim) live.push(this.#scrim);
		return live;
	}
}
