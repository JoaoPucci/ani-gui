/**
 * One tip on screen as a spotlight: the place it points at, where that
 * place and the window's buttons are, and how the tip closes. The host
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
import { isTopModal, openModal, type ModalHandle } from '$lib/components/modal-layer';
import { closing, type CloseRoute } from './closing';
import { roomAbove, type Placement, type Rect } from './place';
import {
	WINDOW_BUTTONS,
	accentAt,
	observePlace,
	readScreen,
	sameRects,
	sameView
} from './screen-read';
import { sameRect } from './spotlight';
import { focusAfterClose } from './tip-focus';

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
	readonly place: HTMLElement;
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
	#modal: ModalHandle | null = null;
	#root: HTMLElement | null = null;
	readonly #unobserve: () => void;
	#scrolled = false;

	constructor(tip: OnboardingTip, place: HTMLElement, deps: SessionDeps) {
		this.tip = tip;
		this.place = place;
		this.#deps = deps;
		this.accent = accentAt(deps.doc, place, this.accent);
		this.#measure();
		// The place may grow, shrink or leave with no scroll to say so:
		// read it again whenever it does.
		this.#unobserve = observePlace(deps.doc, place, () => this.read());
	}

	/** Read where the place, the window and its buttons are now. The
	 *  place gone from the page is a yield. */
	read(): void {
		if (this.closed) return;
		if (!this.place.isConnected) {
			this.close('yield');
			return;
		}
		this.#measure();
	}

	#measure(): void {
		const now = readScreen(this.#deps.doc, this.place);
		if (!sameView(now.view, this.view)) this.view = now.view;
		if (!sameRect(now.rect, this.rect)) this.rect = now.rect;
		if (!sameRects(now.buttons, this.buttons)) this.buttons = now.buttons;
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
		if (this.closed || this.#modal) return;
		this.#root = root;
		this.#shownAt = this.#deps.now();
		const live = [
			this.place,
			...this.#deps.doc.querySelectorAll(WINDOW_BUTTONS),
			...(scrim ? [scrim] : [])
		];
		this.#modal = openModal(root, { onEscape: () => this.close('escape'), live });
	}

	/** A modal opened or closed: one opened over the balloon takes the
	 *  page, and the tip yields to it. */
	modalsChanged(): void {
		// The modal and its root are set together, in hold().
		if (this.#modal && !isTopModal(this.#root as HTMLElement)) this.close('yield');
	}

	/** A press anywhere: one on the place works, and closes the tip. */
	pressed(target: EventTarget | null): void {
		if (target instanceof Node && this.place.contains(target)) this.close('place');
	}

	close(route: CloseRoute): void {
		if (this.closed) return;
		const how = closing(route, this.#shownAt, this.#deps.now());
		if (!how) return;
		this.closed = true;
		this.#unobserve();
		if (how.seen) this.#deps.markSeen(this.tip.id);
		// A tip never held, or held under a dialog now on top, moves no focus.
		const covered = !isTopModal(this.#root ?? this.place);
		// Closing the modal gives focus back to what held it when it opened.
		this.#modal?.close();
		this.#modal = null;
		if (!covered) focusAfterClose(how.focus, this.#deps.doc, this.place, this.#root);
		this.#deps.onClosed(how.seen);
	}
}
