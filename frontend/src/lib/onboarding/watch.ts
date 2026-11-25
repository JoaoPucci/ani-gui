/**
 * Watches the places pending tips point at, and says which have
 * settled on screen: found on the page (`data-onboard="<place>"`), on
 * screen enough to point at (`onScreen`), and unmoved across the
 * IntersectionObserver's thresholds for SETTLE_MS. Rendering glue: an
 * IntersectionObserver per watcher and a MutationObserver for places
 * that mount late, no per-frame reads.
 */
import { SETTLE_MS, onScreen } from './gate';

/** Fine enough that a scroll crosses one every few pixels on a strip,
 *  so "still" means no crossing for SETTLE_MS. */
const THRESHOLDS = Array.from({ length: 21 }, (_, i) => i / 20);

export interface WatchDeps {
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
	readonly #onChange: (settled: ReadonlySet<string>) => void;
	readonly #io: IntersectionObserver;
	readonly #mo: MutationObserver;
	readonly #els = new Map<string, HTMLElement>();
	readonly #timers = new Map<string, ReturnType<typeof setTimeout>>();
	readonly #settled = new Set<string>();

	constructor(places: string[], onChange: (settled: ReadonlySet<string>) => void, deps: WatchDeps) {
		this.#deps = deps;
		this.#places = places;
		this.#onChange = onChange;
		this.#io = deps.observe((entries) => this.#seen(entries), { threshold: THRESHOLDS });
		this.#mo = new MutationObserver(() => this.#find());
		this.#mo.observe(deps.doc.body, {
			childList: true,
			subtree: true,
			attributes: true,
			attributeFilter: ['data-onboard']
		});
		this.#find();
	}

	/** Look for each place on the page, and watch the ones found. */
	#find(): void {
		for (const place of this.#places) {
			const el = placeElement(this.#deps.doc, place);
			const was = this.#els.get(place);
			if (el === was) continue;
			if (was) this.#io.unobserve(was);
			this.#unsettle(place);
			if (el) {
				this.#els.set(place, el);
				this.#io.observe(el);
			} else {
				this.#els.delete(place);
			}
		}
	}

	#placeOf(el: Element): string | null {
		for (const [place, e] of this.#els) if (e === el) return place;
		return null;
	}

	#seen(entries: IntersectionObserverEntry[]): void {
		for (const entry of entries) {
			const place = this.#placeOf(entry.target);
			if (!place) continue;
			// Any crossing is movement: the place settles only once it has
			// stayed on screen, uncrossed, for SETTLE_MS.
			this.#unsettle(place);
			const box = entry.boundingClientRect;
			const view = { h: this.#deps.viewHeight(), top: this.#deps.viewTop() };
			if (!onScreen({ top: box.top, bottom: box.bottom, width: box.width }, view)) continue;
			this.#timers.set(
				place,
				this.#deps.setTimer(() => {
					this.#timers.delete(place);
					this.#settled.add(place);
					this.#onChange(this.#settled);
				}, SETTLE_MS)
			);
		}
	}

	#unsettle(place: string): void {
		const t = this.#timers.get(place);
		if (t !== undefined) this.#deps.clearTimer(t);
		this.#timers.delete(place);
		if (this.#settled.delete(place)) this.#onChange(this.#settled);
	}

	stop(): void {
		this.#io.disconnect();
		this.#mo.disconnect();
		for (const t of this.#timers.values()) this.#deps.clearTimer(t);
		this.#timers.clear();
	}
}
