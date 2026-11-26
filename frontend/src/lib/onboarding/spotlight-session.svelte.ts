/**
 * One tip on screen as a spotlight: its steps ($./tip-steps: one place
 * for a single or entry tip, up to three for a sequence), where the
 * step's place and the window's buttons are, and how the tip closes. The host
 * component ($./OnboardHost.svelte) draws from this and feeds it the
 * screen's events. Rendering glue, kept out of the component so it can
 * be tested.
 *
 * - The balloon becomes the topmost modal once it is measured and
 *   placed ($lib/components/modal-layer): focus moves to its primary
 *   action, Tab stays in it, the app's own shortcuts stand down, and the
 *   rest of the page is inert except the place and the window's buttons,
 *   which a press must still reach.
 * - It closes by the routes $./closing names, and yields, unseen, when
 *   one of the app's dialogs opens over it, the window goes fullscreen,
 *   the page changes or the place leaves it.
 */
import type { OnboardingTip } from '$lib/api';
import { closing, type CloseRoute } from './closing';
import { roomAbove, wholly, type Placement, type Rect } from './place';
import { accentAt, observePlace, rereadInto } from './screen-read';
import { BalloonHold } from './balloon-hold';
import { focusAfterClose } from './tip-focus';
import { TipSteps } from './tip-steps.svelte';

export interface SessionDeps {
	doc: Document;
	now: () => number;
	/** The tip was closed by the person: record it seen. */
	markSeen: (id: string) => void;
	/** The tip closed by any route, seen or not. */
	onClosed: (seen: boolean) => void;
}

export class SpotlightSession {
	readonly tip: OnboardingTip;
	/** Its steps and the one that is up ($./tip-steps). */
	readonly steps: TipSteps;
	/** The place, viewport px, as last read. */
	rect = $state<Rect>({ x: 0, y: 0, w: 0, h: 0 });
	/** The window, and the top bar's bottom edge. */
	view = $state({ w: 0, h: 0, top: 0 });
	/** The window's buttons, viewport px. */
	buttons = $state<Rect[]>([]);
	/** The show's colour at the place. */
	accent = $state('var(--brand)');
	closed = $state(false);

	readonly #deps: SessionDeps;
	/** When the balloon took the page: the press grace counts from here. */
	#shownAt = Number.POSITIVE_INFINITY;
	readonly #hold: BalloonHold;
	#unobserve: () => void;
	#scrolled = false;
	#scroll: ((place: HTMLElement, marginTop: number) => void) | null = null;
	#size = { h: 0 };
	readonly #reread = () => this.read();

	/** `places`: each step's place found on the page, in step order, at
	 *  least one; `numbers`: each one's declared step number. */
	constructor(tip: OnboardingTip, places: HTMLElement[], deps: SessionDeps, numbers?: number[]) {
		this.tip = tip;
		this.steps = new TipSteps(places, numbers);
		this.#deps = deps;
		this.#hold = new BalloonHold(deps.doc, () => this.close('escape'));
		this.accent = accentAt(deps.doc, places[0], this.accent);
		this.#measure();
		// The place may grow, shrink or leave with no scroll to say so:
		// read it again whenever it does.
		this.#unobserve = observePlace(deps.doc, this.place, this.#reread);
	}

	/** The place the step that is up points at. */
	get place(): HTMLElement {
		return this.steps.place;
	}

	/** Next or Back: the step after or before. Next on the last step is
	 *  Done. */
	step(to: 'next' | 'back'): void {
		if (this.closed) return;
		const move = this.steps.go(to);
		if (move === 'done') this.close('done');
		if (move !== 'moved') return;
		this.#unobserve();
		this.#unobserve = observePlace(this.#deps.doc, this.place, this.#reread);
		this.#measure();
		// A step whose place is off screen is brought into room as it
		// comes up.
		if (this.#scroll && !wholly(this.rect, this.view)) {
			this.#scroll(this.place, roomAbove(this.view.top, this.#size));
			this.#measure();
		}
		this.#hold.follow(this.place);
	}

	/** Read where the place, the window and its buttons are now. A
	 *  step's place gone from the page is a yield. */
	read(): void {
		if (this.closed) return;
		if (!this.steps.connected()) {
			this.close('yield');
			return;
		}
		this.#measure();
	}

	#measure(): void {
		rereadInto(this, this.#deps.doc, this.place);
	}

	/** The balloon has its size and its place: a place with no room
	 *  beside it is first scrolled, once, to `roomAbove` under the top
	 *  bar (`scroll` moves the page), and then the balloon takes the
	 *  page. */
	placed(
		placement: Placement,
		size: { h: number },
		balloon: HTMLElement,
		scrim: HTMLElement | null,
		scroll: (place: HTMLElement, marginTop: number) => void
	): void {
		if (this.closed) return;
		this.#scroll = scroll;
		this.#size = size;
		if (!placement.fits && !this.#scrolled) {
			this.#scrolled = true;
			scroll(this.place, roomAbove(this.view.top, size));
			this.#measure();
		}
		this.hold(balloon, scrim);
	}

	/** The balloon is measured and placed: it becomes the topmost modal.
	 *  `scrim` stays within reach too, so a press outside lands on its
	 *  catcher rather than on an inert page. */
	hold(root: HTMLElement, scrim: HTMLElement | null = null): void {
		if (this.closed || this.#hold.held) return;
		this.#shownAt = this.#deps.now();
		this.#hold.take(root, scrim, this.place);
	}

	/** A modal opened or closed: one opened over the balloon takes the
	 *  page, and the tip yields to it. */
	modalsChanged(): void {
		if (this.#hold.covered()) this.close('yield');
	}

	/** A press anywhere: one on the place works, and closes the tip. */
	pressed(target: EventTarget | null): void {
		if (this.place.contains(target as Node | null)) this.close('place');
	}

	close(route: CloseRoute): void {
		if (this.closed) return;
		const how = closing(route, this.#shownAt, this.#deps.now());
		if (!how) return;
		this.closed = true;
		this.#unobserve();
		if (how.seen) this.#deps.markSeen(this.tip.id);
		// Closing the modal gives focus back to what held it when it
		// opened. A tip never held, or under a dialog now on top, moves no
		// focus itself.
		const { covered } = this.#hold.release();
		if (!covered) focusAfterClose(how.focus, this.#deps.doc, this.place, this.#hold.root);
		this.#deps.onClosed(how.seen);
	}
}
