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
import type { Rect } from './place';
import { sameRect } from './spotlight';

/** The window's own buttons: minimize, maximize, close. */
const WINDOW_BUTTONS = '.winctl-btn';
const FOCUSABLE = 'a[href], button:not([disabled]), input, select, [tabindex]:not([tabindex="-1"])';

export interface SessionDeps {
	doc: Document;
	now: () => number;
	/** The tip was closed by the person: record it seen. */
	markSeen: (id: string) => void;
	/** The tip closed by any route, seen or not. */
	onClosed: (seen: boolean) => void;
}

const toRect = (r: DOMRect): Rect => ({ x: r.left, y: r.top, w: r.width, h: r.height });

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
	readonly #shownAt: number;
	#modal: ModalHandle | null = null;
	#root: HTMLElement | null = null;

	constructor(tip: OnboardingTip, place: HTMLElement, deps: SessionDeps) {
		this.tip = tip;
		this.place = place;
		this.#deps = deps;
		this.#shownAt = deps.now();
		const a = deps.doc.defaultView?.getComputedStyle(place).getPropertyValue('--accent').trim();
		if (a) this.accent = a;
		this.read();
	}

	/** Read where the place, the window and its buttons are now. The
	 *  place gone from the page is a yield. */
	read(): void {
		if (this.closed) return;
		if (!this.place.isConnected) {
			this.close('yield');
			return;
		}
		const doc = this.#deps.doc;
		const win = doc.defaultView;
		const top = doc.querySelector('.topbar')?.getBoundingClientRect().bottom ?? 0;
		const view = { w: win?.innerWidth ?? 0, h: win?.innerHeight ?? 0, top: Math.round(top) };
		if (view.w !== this.view.w || view.h !== this.view.h || view.top !== this.view.top) {
			this.view = view;
		}
		const rect = toRect(this.place.getBoundingClientRect());
		if (!sameRect(rect, this.rect)) this.rect = rect;
		const buttons = [...doc.querySelectorAll<HTMLElement>(WINDOW_BUTTONS)].map((b) =>
			toRect(b.getBoundingClientRect())
		);
		if (
			buttons.length !== this.buttons.length ||
			buttons.some((b, i) => !sameRect(b, this.buttons[i]))
		) {
			this.buttons = buttons;
		}
	}

	/** The balloon is measured and placed: it becomes the topmost modal.
	 *  `scrim` stays within reach too, so a press outside lands on its
	 *  catcher rather than on an inert page. */
	hold(root: HTMLElement, scrim: HTMLElement | null = null): void {
		if (this.closed || this.#modal) return;
		this.#root = root;
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
		if (this.#root && this.#modal && !isTopModal(this.#root)) this.close('yield');
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
		if (how.seen) this.#deps.markSeen(this.tip.id);
		const covered = this.#root !== null && !isTopModal(this.#root);
		// Closing the modal gives focus back to what held it when it opened.
		this.#modal?.close();
		this.#modal = null;
		if (!covered) this.#focus(how.focus);
		this.#deps.onClosed(how.seen);
	}

	#focus(to: 'place' | 'back' | 'none'): void {
		if (to === 'none') return;
		const active = this.#deps.doc.activeElement;
		const nowhere = !active || active === this.#deps.doc.body;
		if (to === 'back' && !nowhere) return;
		const target = this.place.matches(FOCUSABLE)
			? this.place
			: this.place.querySelector<HTMLElement>(FOCUSABLE);
		target?.focus({ preventScroll: true });
	}
}
