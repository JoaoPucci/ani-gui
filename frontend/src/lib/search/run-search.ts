/**
 * The /search page's query flow, extracted per AGENTS.md §2 so the
 * component stays a thin adapter. One runner instance owns the
 * page's async state transitions: config load, the Kitsu search a
 * page at a time, and the progressive availability filter's
 * emissions for each page.
 *
 * Every run is its own search, and a superseded one is silenced by
 * identity, NOT by comparing query text: navigating A → B → A starts
 * two distinct searches for the text "A", and a text guard would let
 * the first one's late emissions — or a later page of it still in
 * flight — overwrite the second's results and clear its spinner
 * early. Every callback checks that its search is still the current
 * one before touching page state.
 *
 * Paging: each page goes through the filter on its own and renders
 * as its own slice of the grid, so a late prune on one page never
 * disturbs another. What follows a page's first render — settle,
 * end, load the next page by itself, or hold for the user — is the
 * pure rule in `paging.ts`.
 */

import type { Config, KitsuAnimeRef, KitsuSearchPage, SearchOrder } from '$lib/api';
import { afterPage, canLoadMore, freshHits, type PagingPhase } from './paging';
import { probeScope, type ProbeRun } from '$lib/availability/probe-scope';
import {
	canReuse,
	resumeStep,
	type ResumeTarget,
	type SearchCheckpoint,
	type SearchSnapshot
} from './snapshot';

export interface SearchRunnerDeps {
	/** One page of the Kitsu search in `order`, starting at `offset`. */
	searchPage: (q: string, offset: number, order: SearchOrder) => Promise<KitsuSearchPage>;
	/** Settings fetch; rejections fall back to a null config (the
	 *  mode picker defaults to 'sub'), never to the error state. */
	getConfig: () => Promise<Config | null>;
	pickMode: (config: Config | null) => 'sub' | 'dub';
	/** The progressive availability filter: emits the visible list at
	 *  the grace deadline and again on each late prune. */
	filter: (
		items: KitsuAnimeRef[],
		mode: 'sub' | 'dub',
		emit: (visible: KitsuAnimeRef[]) => void,
		/** Aborts the page's probes; the run's own, so a run that is
		 *  superseded can stop them. */
		signal?: AbortSignal
	) => Promise<void>;
	onResults: (visible: KitsuAnimeRef[]) => void;
	onError: (e: unknown) => void;
	onBusy: (busy: boolean) => void;
	/** Every change of the paging phase. */
	onPhase: (phase: PagingPhase) => void;
	/** A resumed search has its results up: whether the card the reader
	 *  left at the top of the screen is among them. */
	onReached?: (found: boolean) => void;
}

/** One run's search: the pages it has shown and where it goes next. */
interface Search {
	q: string;
	order: SearchOrder;
	mode: 'sub' | 'dub';
	/** Each page's visible hits, in page order. */
	pages: KitsuAnimeRef[][];
	/** Every id a page has carried, shown or not. */
	seen: Set<string>;
	/** The offset of the next page; `null` once there is none. */
	next: number | null;
	emptyStreak: number;
	/** Pages loaded, counting any a resume brought back. */
	loaded: number;
	/** The probes this search's pages ask: ended when another search
	 *  replaces it, since the page's path does not change. */
	probes: ProbeRun;
	/** Where it stood when its last page settled; null before then. */
	settled: Omit<SearchCheckpoint, 'hits'> | null;
	/** A search asked again on the way back: the card it loads pages
	 *  until it shows. */
	target: ResumeTarget | null;
}

export function createSearchRunner(deps: SearchRunnerDeps): {
	/** Start a search for `q` in `order` (Kitsu's ranking by default). */
	run: (q: string, order?: SearchOrder) => Promise<void>;
	/** Load the next page: when idle, retrying a failed page, or
	 *  continuing a held search. Does nothing in any other phase. */
	loadMore: () => void;
	/** Where the current search stood when its last page settled, with
	 *  the results it shows; `null` before a first page has settled. */
	checkpoint: () => SearchCheckpoint | null;
	/** Bring back a search the page saved when the reader left it. */
	resume: (saved: SearchSnapshot, at: { now: number; reload: boolean }) => Promise<void>;
} {
	let generation = 0;
	let previousProbes: ProbeRun | null = null;
	let search: Search | null = null;
	let phase: PagingPhase = { kind: 'loading' };
	// Config is cached only after a SUCCESSFUL load, and the load is
	// SHARED: concurrent runs await one in-flight getConfig instead
	// of racing their own. Both properties close the same trap — a
	// failure (a deeplink run losing the race to the mount's settings
	// request, or a stale concurrent load rejecting after a newer one
	// succeeded) must never pin the 'sub' fallback for the mount; the
	// next run simply loads again.
	let config: Config | null = null;
	let configLoaded = false;
	let configInFlight: Promise<void> | null = null;

	const loadConfigOnce = () => {
		configInFlight ??= deps
			.getConfig()
			.then((c) => {
				config = c;
				configLoaded = true;
			})
			.catch(() => {})
			.finally(() => {
				configInFlight = null;
			});
		return configInFlight;
	};

	const setPhase = (next: PagingPhase) => {
		phase = next;
		deps.onPhase(next);
	};

	/** Fetch `s`'s next page and run it through the filter. Rejects
	 *  only when the page itself could not be fetched. */
	const fetchPage = async (s: Search) => {
		const page = await deps.searchPage(s.q, s.next ?? 0, s.order);
		// A superseded search still runs its page through the filter,
		// as it always has; its emissions are what the guard drops.
		s.next = page.next_offset;
		s.loaded += 1;
		const index = s.pages.push([]) - 1;
		let settled = false;
		await deps.filter(
			freshHits(s.seen, page.hits),
			s.mode,
			(visible) => {
				if (s !== search) return;
				s.pages[index] = visible;
				deps.onResults(s.pages.flat());
				deps.onBusy(false);
				if (!settled) {
					settled = true;
					settle(s, visible.length);
				}
			},
			s.probes.signal
		);
	};

	const settle = (s: Search, visible: number) => {
		const r = afterPage(visible, s.next, s.emptyStreak);
		s.emptyStreak = r.emptyStreak;
		s.settled = {
			mode: s.mode,
			seen: [...s.seen],
			next: s.next,
			emptyStreak: r.emptyStreak,
			pages: s.loaded,
			phase: r.phase.kind === 'end' || r.phase.kind === 'held' ? r.phase.kind : 'idle'
		};
		if (s.target) {
			const at = { shown: s.pages.flat(), pages: s.loaded, phase: r.phase.kind };
			const step = resumeStep(s.target, at);
			if (step === 'more') {
				void fetchMore(s);
				return;
			}
			if (step !== 'wait') {
				s.target = null;
				setPhase(r.phase);
				deps.onReached?.(step === 'found');
				if (r.advance) void fetchMore(s);
				return;
			}
		}
		setPhase(r.phase);
		if (r.advance) void fetchMore(s);
	};

	const fetchMore = async (s: Search) => {
		setPhase({ kind: 'loading-more' });
		try {
			await fetchPage(s);
		} catch (error) {
			if (s !== search) return;
			setPhase({ kind: 'failed', error });
			// The card cannot be reached past a page that failed; the
			// reader can retry the page from where the search stands.
			if (s.target) {
				s.target = null;
				deps.onReached?.(false);
			}
		}
	};

	/**
	 * Start a new search, replacing the current one: its probes, phase
	 * and busy state end, the config loads if it has not, and `body`
	 * runs with the probes for its pages — unless another search has
	 * replaced this one while the config loaded, in which case nothing
	 * is sent: the search that replaced it owns the page now.
	 */
	const launch = async (body: (probes: ProbeRun) => Promise<void> | void) => {
		const gen = ++generation;
		const current = () => gen === generation;
		// The search this one replaces stops asking: its emissions were
		// already dropped, and its probes would hold the limiter's slots.
		previousProbes?.end();
		const probes = probeScope.begin();
		previousProbes = probes;
		search = null;
		setPhase({ kind: 'loading' });
		deps.onBusy(true);
		try {
			if (!configLoaded) await loadConfigOnce();
			if (current()) await body(probes);
		} catch (e) {
			if (current()) deps.onError(e);
		} finally {
			if (current()) deps.onBusy(false);
		}
	};

	/** A search for `q` from its first page, made the current one. */
	const begin = (q: string, order: SearchOrder, probes: ProbeRun, target: ResumeTarget | null) =>
		(search = {
			q,
			order,
			mode: deps.pickMode(config),
			pages: [],
			seen: new Set(),
			next: 0,
			emptyStreak: 0,
			loaded: 0,
			probes,
			settled: null,
			target
		});

	const run = (q: string, order: SearchOrder = 'relevance') =>
		launch((probes) => fetchPage(begin(q, order, probes, null)));

	const checkpoint = (): SearchCheckpoint | null =>
		search?.settled ? { ...search.settled, hits: search.pages.flat() } : null;

	/** The saved search made the current one as it was, its results
	 *  shown without asking Kitsu for anything. */
	const restore = (saved: SearchSnapshot, kept: SearchCheckpoint, probes: ProbeRun) => {
		const { hits, ...settled } = kept;
		const s = begin(saved.q, saved.order, probes, null);
		Object.assign(s, {
			pages: [hits],
			seen: new Set(kept.seen),
			next: kept.next,
			emptyStreak: kept.emptyStreak,
			loaded: kept.pages,
			settled
		});
		deps.onResults(hits);
		setPhase({ kind: kept.phase });
		const anchor = saved.anchor;
		deps.onReached?.(anchor !== null && hits.some((h) => h.id === anchor.id));
	};

	/**
	 * Bring back a saved search: as it was, when `canReuse` allows;
	 * otherwise run again in the saved order, loading pages until the
	 * card the reader left at the top shows or the pages they had are
	 * loaded. Either way `onReached` says whether that card is up.
	 */
	const resume = (saved: SearchSnapshot, at: { now: number; reload: boolean }) =>
		launch((probes) => {
			const kept = saved.results;
			if (kept && canReuse(saved, { ...at, mode: deps.pickMode(config) })) {
				return restore(saved, kept, probes);
			}
			const target = { id: saved.anchor?.id ?? null, pages: Math.max(1, saved.pages) };
			return fetchPage(begin(saved.q, saved.order, probes, target));
		});

	const loadMore = () => {
		if (!search || !canLoadMore(phase)) return;
		if (phase.kind === 'held') search.emptyStreak = 0;
		void fetchMore(search);
	};

	return { run, loadMore, checkpoint, resume };
}
