/**
 * The engine under the search runner (`run-search.ts`): the current
 * search and its paging phase, fetching a page and running it through
 * the availability filter, what follows a page's first render, and
 * starting a search that replaces the current one. The runner composes
 * its operations — run, load more, checkpoint, resume, stop — from
 * these.
 */

import type { Config, KitsuAnimeRef, KitsuSearchPage, SearchOrder } from '$lib/api';
import { afterPage, freshHits, type PagingPhase } from './paging';
import { probeScope, type ProbeRun } from '$lib/availability/probe-scope';
import { resumeStep, type ResumeTarget } from './snapshot';
import { configOnce } from './config-once';
import { checkpointAt, type Settled } from './resume';
import { searchTurns } from './turns';

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
	/** The clock; `Date.now` unless a test gives one. */
	now?: () => number;
}

/** One run's search: the pages it has shown and where it goes next. */
export interface Search {
	/** When its first page was fetched; a restored search keeps the
	 *  saved one. */
	fetchedAt: number;
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
	settled: Settled | null;
	/** A search asked again on the way back: the card it loads pages
	 *  until it shows. */
	target: ResumeTarget | null;
}

export function createSearchCore(deps: SearchRunnerDeps) {
	/** The current search, and its paging phase. */
	const state: { search: Search | null; phase: PagingPhase } = {
		search: null,
		phase: { kind: 'loading' }
	};
	// Each search takes a turn; the one it replaces stops asking
	// ($lib/search/turns).
	const turns = searchTurns(() => probeScope.begin());
	// Loaded once and shared across runs ($lib/search/config-once).
	const settings = configOnce(deps.getConfig);

	const setPhase = (next: PagingPhase) => {
		state.phase = next;
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
				if (s !== state.search) return;
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
		s.settled = checkpointAt(s, r);
		const at = { shown: s.pages.flat(), pages: s.loaded, phase: r.phase.kind };
		const step = resumeStep(s.target, at);
		if (step === 'more') return void fetchMore(s);
		setPhase(r.phase);
		if (step !== 'wait') reached(s, step === 'found');
		if (r.advance) void fetchMore(s);
	};

	/** A resumed search is done aiming for its card. */
	const reached = (s: Search, found: boolean) => {
		s.target = null;
		deps.onReached?.(found);
	};

	const fetchMore = async (s: Search) => {
		setPhase({ kind: 'loading-more' });
		try {
			await fetchPage(s);
		} catch (error) {
			if (s !== state.search) return;
			setPhase({ kind: 'failed', error });
			// The card cannot be reached past a page that failed; the
			// reader can retry the page from where the search stands.
			if (s.target) reached(s, false);
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
		const { current, probes } = turns.take();
		state.search = null;
		setPhase({ kind: 'loading' });
		deps.onBusy(true);
		try {
			await settings.ready();
			if (current()) await body(probes);
		} catch (e) {
			if (current()) deps.onError(e);
		} finally {
			if (current()) deps.onBusy(false);
		}
	};

	/** A search for `q` from its first page, made the current one. */
	const begin = (q: string, order: SearchOrder, probes: ProbeRun, target: ResumeTarget | null) =>
		(state.search = {
			q,
			order,
			mode: deps.pickMode(settings.config),
			pages: [],
			seen: new Set(),
			next: 0,
			emptyStreak: 0,
			loaded: 0,
			fetchedAt: (deps.now ?? Date.now)(),
			probes,
			settled: null,
			target
		});

	return { state, settings, turns, setPhase, fetchPage, fetchMore, launch, begin };
}
