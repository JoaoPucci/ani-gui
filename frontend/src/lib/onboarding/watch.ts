/**
 * Watches the places pending tips point at, and says which have
 * settled on screen ($./settle): found on the page
 * (`data-onboard="<place>"`), on screen enough to point at (`onScreen`),
 * and unmoved for SETTLE_MS. Rendering glue: an IntersectionObserver
 * sees a place cross into or out of view, a scroll anywhere is a move
 * of every place it watches (a place fully in view crosses no
 * threshold while the page scrolls under it), and a MutationObserver
 * finds places that mount late. No per-frame reads.
 */
import { onScreen } from './gate';
import { Settler, type Timers } from './settle';

/** Fine enough that a scroll crosses one every few pixels on a strip,
 *  so "still" means no crossing for SETTLE_MS. */
const THRESHOLDS = Array.from({ length: 21 }, (_, i) => i / 20);

export interface WatchDeps extends Timers {
	doc: Document;
	/** The top bar's bottom edge, in viewport px. */
	viewTop: () => number;
	/** The window's height, in viewport px. */
	viewHeight: () => number;
	observe: (
		cb: IntersectionObserverCallback,
		init: IntersectionObserverInit
	) => IntersectionObserver;
	setTimer: (fn: () => void, ms: number) => ReturnType<typeof setTimeout>;
	clearTimer: (t: ReturnType<typeof setTimeout>) => void;
}

export function browserWatchDeps(doc: Document = document): WatchDeps {
	return {
		doc,
		viewTop: () => doc.querySelector('.topbar')?.getBoundingClientRect().bottom ?? 0,
		viewHeight: () => doc.defaultView?.innerHeight ?? 0,
		observe: (cb, init) => new IntersectionObserver(cb, init),
		setTimer: (fn, ms) => setTimeout(fn, ms),
		clearTimer: (t) => clearTimeout(t)
	};
}

/** The element marked as `place`, if the page has one. Places are
 *  kebab-case (the registry holds them to it), so need no escaping. */
export function placeElement(doc: Document, place: string): HTMLElement | null {
	return doc.querySelector<HTMLElement>(`[data-onboard="${place}"]`);
}

export class PlaceWatcher {
	readonly #deps: WatchDeps;
	readonly #places: string[];
	readonly #io: IntersectionObserver;
	readonly #mo: MutationObserver;
	readonly #els = new Map<string, HTMLElement>();
	readonly #settler: Settler;
	readonly #onScroll = () => this.#moved();

	constructor(places: string[], onChange: (settled: ReadonlySet<string>) => void, deps: WatchDeps) {
		this.#deps = deps;
		this.#places = places;
		this.#settler = new Settler(onChange, deps);
		this.#io = deps.observe((entries) => this.#seen(entries), { threshold: THRESHOLDS });
		this.#mo = new MutationObserver(() => this.#find());
		this.#mo.observe(deps.doc.body, {
			childList: true,
			subtree: true,
			attributes: true,
			attributeFilter: ['data-onboard']
		});
		deps.doc.addEventListener('scroll', this.#onScroll, { capture: true, passive: true });
		this.#find();
	}

	/** Look for each place on the page, and watch the ones found. */
	#find(): void {
		for (const place of this.#places) {
			const el = placeElement(this.#deps.doc, place);
			const was = this.#els.get(place);
			if (el === was) continue;
			if (was) this.#io.unobserve(was);
			this.#settler.drop(place);
			if (el) {
				this.#els.set(place, el);
				this.#io.observe(el);
			} else {
				this.#els.delete(place);
			}
		}
	}

	#visible(box: { top: number; bottom: number; width: number }): boolean {
		return onScreen(box, { h: this.#deps.viewHeight(), top: this.#deps.viewTop() });
	}

	/** Crossings: each is a sighting of the place it names. */
	#seen(entries: IntersectionObserverEntry[]): void {
		for (const entry of entries) {
			for (const [place, el] of this.#els) {
				if (el === entry.target)
					this.#settler.sighted(place, this.#visible(entry.boundingClientRect));
			}
		}
	}

	/** A scroll moved the page: every place starts settling again. */
	#moved(): void {
		for (const [place, el] of this.#els) {
			this.#settler.sighted(place, this.#visible(el.getBoundingClientRect()));
		}
	}

	stop(): void {
		this.#io.disconnect();
		this.#mo.disconnect();
		this.#deps.doc.removeEventListener('scroll', this.#onScroll, { capture: true });
		this.#settler.stop();
	}
}
