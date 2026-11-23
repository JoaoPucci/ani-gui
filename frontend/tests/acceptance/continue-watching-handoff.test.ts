// Acceptance: a Continue card whose finale has been played through moves
// on to what comes next in the series. The card takes the finished
// show's place, says "Next", names both shows, and opens the next
// show's page; it does not start playback. It moves on only to one
// aired show, in the order the Watch order setting chooses, that has
// no history row of its own; otherwise the row keeps its ordinary
// card. Only a row at such a finale asks for its series. A show outside
// the main story moves on along its own chain.
//
// The backend decides all of it: the rail (`POST /api/watch-order/rail`,
// answered by its fake, `railHandler`) says which rows ask and which
// show a hand-off, and each asking series' hand-off (`POST
// /api/watch-order/:id/handoff`) is the answer the backend recorded for
// the franchise the walk answers, the rows the resume lookup names and
// the order setting. These scenarios assert what Home draws from those
// answers and which reads it sends; the decisions are asserted by name
// in backend/src/watch_order/scenario_handoff_test.rs.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { appConfig, homeHandlers, kitsuRef } from './home-handlers';
import { watchPageFetches } from './page-fetches';
import { availabilityAnswer, errorAnswer } from '../answers';
import { answerResponse, railHandler, type ScenarioInputs } from './watch-order-answers';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));
vi.mock('$app/navigation', () => ({
	goto: vi.fn(async () => {}),
	invalidateAll: vi.fn(async () => {}),
	beforeNavigate: vi.fn(),
	afterNavigate: vi.fn()
}));

import HomePage from '../../src/routes/+page.svelte';
import {
	__resetApiBaseForTests,
	type AvailabilityResponse,
	type Config,
	type Franchise,
	type FranchiseEntry,
	type KitsuAnimeRef
} from '../../src/lib/api';
import { m } from '../../src/lib/paraglide/messages';
import { savePosition } from '../../src/lib/play/watch-position';

const FIRST = 'Arc of the Moon: First Night';
const SECOND = 'Arc of the Moon: Second Night';
const BRANCH = 'Arc of the Moon: Other Night';
// A second series whose finale always moves on: a scenario that
// expects no hand-off waits for this one, so it knows the reads it
// cares about have settled and rendered.
const CONTROL = 'Tide Bell: Spring';
const CONTROL_NEXT = 'Tide Bell: Summer';

const entry = (id: string, title: string, start: string, episodes: number): FranchiseEntry => ({
	id,
	title,
	subtype: 'TV',
	start,
	episodes,
	status: 'finished',
	poster: null,
	cover: null
});

const FIRST_E = entry('10', FIRST, '2020-01-01', 13);
const SECOND_E = entry('20', SECOND, '2021-01-01', 13);
const BRANCH_E = entry('25', BRANCH, '2022-01-01', 12);

const lineFrom10: Franchise = {
	anchor: '10',
	name: FIRST,
	name_from: '10',
	entries: [FIRST_E, SECOND_E],
	links: [['10', '20']],
	versions: [],
	films: [],
	sides: [],
	recaps: [],
	parents: {},
	complete: true
};
// As linked, the first show leads two ways; as released, to the
// earlier one.
const forkFrom10: Franchise = {
	...lineFrom10,
	entries: [FIRST_E, SECOND_E, BRANCH_E],
	links: [
		['10', '20'],
		['10', '25']
	]
};

const lineFrom30: Franchise = {
	...lineFrom10,
	anchor: '30',
	name: CONTROL,
	name_from: '30',
	entries: [entry('30', CONTROL, '2015-01-01', 13), entry('40', CONTROL_NEXT, '2016-01-01', 13)],
	links: [['30', '40']]
};
const controlRow = () => row('13', 'p-control', '30', CONTROL);

const row = (ep_no: string, id = 'p-first', kitsu_id = '10', title = FIRST) => ({
	ep_no,
	id,
	title,
	kitsu_id
});

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	localStorage.clear();
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	localStorage.clear();
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

const SUITE = 'continue-watching-handoff';

/** The walk's answer: a franchise, one held until it lands, or a failure. */
type Walk = Franchise | 'fail';

interface Scenario {
	history: ReturnType<typeof row>[];
	/** The first show's series, as the walk answers it; `lineFrom10` by default. */
	franchise?: () => Walk | Promise<Walk>;
	order?: Config['watch_order'];
	/** The settings read's answer; the Watch order `order` by default. */
	settings?: () => Response;
	/** What the source's availability probe counts for every show. */
	probedCap?: number;
	/**
	 * The rows the backend's resume lookup names for an id, where it
	 * is not the row that recorded it (`history_claim::names`).
	 */
	named?: Record<string, ReturnType<typeof row>>;
	/** The resume lookup fails: the hand-off reads answer as the backend does then. */
	namedFails?: boolean;
}

/**
 * Mounts Home over `history`, the first show's series answered by
 * `franchise`. Returns the ids whose hand-off Home read, in order.
 */
function mountHome(s: Scenario) {
	const asked: string[] = [];
	const removed = new Set<string>();
	// What the resume lookup names for each id, among the rows still in
	// history: the scenario's named row, else the row that recorded it;
	// none when the lookup cannot be read.
	const resumes = (): Record<string, string> | null => {
		if (s.namedFails) return null;
		const out: Record<string, string> = {};
		for (const r of s.history)
			if (r.kitsu_id && !removed.has(r.id) && !(r.kitsu_id in out)) out[r.kitsu_id] = r.ep_no;
		for (const [id, r] of Object.entries(s.named ?? {})) if (!removed.has(r.id)) out[id] = r.ep_no;
		return out;
	};
	const walk = (id: string): Walk | Promise<Walk> => {
		if (id === '30') return lineFrom30;
		if (id !== '10') return 'fail';
		return s.franchise ? s.franchise() : lineFrom10;
	};
	server.use(
		...homeHandlers({ history: s.history }, [
			http.get(`${API_BASE}/api/settings`, () =>
				s.settings
					? s.settings()
					: HttpResponse.json<Config>({ ...appConfig(), watch_order: s.order ?? 'linked' })
			),
			http.post(`${API_BASE}/api/availability`, () =>
				HttpResponse.json<AvailabilityResponse>(
					availabilityAnswer({
						available: true,
						episode_count: s.probedCap ?? 13
					})
				)
			),
			http.get(`${API_BASE}/api/kitsu/anime/30`, () =>
				HttpResponse.json<KitsuAnimeRef>(kitsuRef('30', CONTROL, 13))
			),
			// A row that recorded no Kitsu id is matched by its title.
			http.post(`${API_BASE}/api/kitsu/search`, async ({ request }) => {
				const body = JSON.stringify(await request.json());
				return HttpResponse.json<KitsuAnimeRef[]>(
					body.includes('Second') ? [kitsuRef('20', SECOND, 13)] : []
				);
			}),
			http.get(`${API_BASE}/api/kitsu/anime/10`, () =>
				HttpResponse.json<KitsuAnimeRef>(kitsuRef('10', FIRST, 13))
			),
			http.get(`${API_BASE}/api/kitsu/anime/20`, () =>
				HttpResponse.json<KitsuAnimeRef>(kitsuRef('20', SECOND, 13))
			),
			// An entry Kitsu has deleted: a row that recorded it is
			// matched again by its title.
			http.get(`${API_BASE}/api/kitsu/anime/99`, () =>
				errorAnswer(502, { kind: 'upstream', status: 404, key: 'error.network.upstream' })
			),
			// A row the scenario's own show stands for, matched by its id.
			http.get(`${API_BASE}/api/kitsu/anime/77`, () =>
				HttpResponse.json<KitsuAnimeRef>(kitsuRef('77', OTHER, 13))
			),
			// Removing a row: the backend's history no longer holds it.
			http.delete(`${API_BASE}/api/history/:id`, ({ params }) => {
				removed.add(String(params.id));
				return new HttpResponse(null, { status: 204 });
			}),
			railHandler(SUITE),
			// A series' hand-off: the backend walks the franchise and asks
			// its resume lookup about the entry it would move on to.
			http.post(`${API_BASE}/api/watch-order/:id/handoff`, async ({ params }) => {
				const id = String(params.id);
				asked.push(id);
				const w = await walk(id);
				if (w === 'fail') return new HttpResponse(null, { status: 503 });
				const inputs: ScenarioInputs = {
					route: 'handoff',
					id,
					order: s.order ?? 'linked',
					franchise: w,
					resumes: resumes()
				};
				return answerResponse(SUITE, inputs);
			})
		])
	);
	app = mount(HomePage, { target });
	return asked;
}

/** Whether `predicate` held after any change to the page since mount, not only at the end. */
function everHeld(predicate: () => boolean): () => boolean {
	let held = predicate();
	const observer = new MutationObserver(() => {
		if (predicate()) held = true;
	});
	observer.observe(target, { subtree: true, childList: true, attributes: true });
	return () => held;
}

/**
 * Until the reads a no-hand-off case cares about have answered and
 * rendered: the control series has moved on (so the order and the
 * played set are known), the first series has been asked for, and a
 * few more turns have passed for its answer to land.
 */
async function settled(asked: readonly string[]) {
	await until(() => handoffTo('40') !== null, 'the other series to move on');
	await until(() => asked.includes('10'), "the first show's series read");
	await new Promise((r) => setTimeout(r, 50));
}

const OTHER = 'Something Else';

/** Removes the card titled `title` as a viewer does: its remove chip, then the confirm. */
async function removeCard(title: string) {
	const button = (pred: (b: HTMLButtonElement) => boolean) =>
		Array.from(target.querySelectorAll('button')).find(pred) ?? null;
	const chip = button(
		(b) => b.getAttribute('aria-label') === m.home_delete_card_aria_label({ title })
	);
	expect(chip, 'the remove chip').not.toBeNull();
	chip!.click();
	await until(
		() => button((b) => (b.textContent ?? '').includes(m.home_delete_confirm_button())) !== null,
		'the remove confirmation'
	);
	button((b) => (b.textContent ?? '').includes(m.home_delete_confirm_button()))!.click();
}

const handoffTo = (id: string) =>
	target.querySelector<HTMLAnchorElement>(`a[href$="/anime/${id}"]`);
const resumeFor = (title: string) =>
	Array.from(target.querySelectorAll('button.resume-card')).find((b) =>
		b.textContent?.includes(title)
	) ?? null;

describe('Continue Watching after a finale', () => {
	it("moves the finished show's card on to the next show's page", async () => {
		const asked = mountHome({ history: [row('13')] });
		await until(() => handoffTo('20') !== null, 'the card to move on to the next show');

		const card = handoffTo('20')!;
		expect(card.getAttribute('aria-label')).toBe(
			m.watchorder_handoff_aria({ title: SECOND, finished: FIRST })
		);
		expect(card.textContent).toContain(m.watchorder_handoff_next());
		expect(card.textContent).toContain(SECOND);
		// The finished show by its whole title, as the rest of the app
		// names it: no words cut because the next show shares them.
		expect(card.querySelector('.show')?.textContent).toBe(
			m.watchorder_handoff_after({ title: FIRST })
		);
		// The hand-off takes the finished show's place: no card offers
		// its finale again.
		expect(resumeFor(FIRST)).toBeNull();
		expect(target.querySelectorAll('.resume-cell')).toHaveLength(1);
		// Removing the card removes the finished show's row, as before.
		expect(
			target.querySelector(
				`button[aria-label="${m.home_delete_card_aria_label({ title: FIRST })}"]`
			)
		).not.toBeNull();
		expect(asked).toEqual(['10']);
	});

	it('moves a show outside the main story on along its own chain', async () => {
		const main = entry('5', 'Arc of the Moon', '2019-01-01', 13);
		mountHome({
			history: [row('13')],
			franchise: () => ({
				...lineFrom10,
				entries: [main],
				links: [],
				chain: {
					entries: [main, FIRST_E, SECOND_E],
					links: [
						['5', '10'],
						['10', '20']
					],
					seams: ['5']
				}
			})
		});
		await until(() => handoffTo('20') !== null, 'the card to move on along the chain');
		expect(handoffTo('20')!.getAttribute('aria-label')).toBe(
			m.watchorder_handoff_aria({ title: SECOND, finished: FIRST })
		);
	});

	it('keeps both ordinary cards once the next show has a row of its own', async () => {
		const asked = mountHome({
			history: [row('3', 'p-second', '20', SECOND), row('13'), controlRow()]
		});
		const movedOn = everHeld(() => handoffTo('20') !== null);
		await settled(asked);
		await until(() => resumeFor(SECOND) !== null, "the next show's own card");
		expect(movedOn()).toBe(false);
		expect(resumeFor(FIRST)).not.toBeNull();
	});

	it('asks for no series while the show is short of its finale', async () => {
		const asked = mountHome({ history: [row('12')] });
		await until(() => resumeFor(FIRST) !== null, 'the ordinary card');
		await new Promise((r) => setTimeout(r, 50));
		expect(asked).toEqual([]);
		expect(handoffTo('20')).toBeNull();
	});

	it('keeps offering a finale left part-way, asking for no series', async () => {
		savePosition('10', 13, 600, 1400);
		const asked = mountHome({ history: [row('13')] });
		await until(() => resumeFor(FIRST) !== null, 'the ordinary card');
		await new Promise((r) => setTimeout(r, 50));
		expect(asked).toEqual([]);
		expect(handoffTo('20')).toBeNull();
	});

	it('keeps the ordinary card, with no error, when the series cannot be read', async () => {
		const asked = mountHome({
			history: [row('13'), controlRow()],
			franchise: () => 'fail'
		});
		const movedOn = everHeld(() => handoffTo('20') !== null);
		await settled(asked);
		expect(movedOn()).toBe(false);
		expect(resumeFor(FIRST)).not.toBeNull();
	});

	it('keeps the ordinary card where the show leads two ways as linked', async () => {
		const asked = mountHome({
			history: [row('13'), controlRow()],
			franchise: () => forkFrom10
		});
		const movedOn = everHeld(() => handoffTo('20') !== null || handoffTo('25') !== null);
		await settled(asked);
		expect(resumeFor(FIRST)).not.toBeNull();
		expect(movedOn()).toBe(false);
	});

	it('moves on to the next show by release date when the setting says so', async () => {
		mountHome({
			history: [row('13')],
			franchise: () => forkFrom10,
			order: 'released'
		});
		await until(() => handoffTo('20') !== null, 'the card to move on as released');
		expect(handoffTo('25')).toBeNull();
	});

	it('keeps the ordinary card when the next show was played from a row with no recorded id', async () => {
		// Written before history recorded the Kitsu id: the row is
		// matched by its title, and counts as played once it is.
		const second = { ep_no: '2', id: 'p-second', title: SECOND };
		mountHome({ history: [second as ReturnType<typeof row>, row('13'), controlRow()] });
		const movedOn = everHeld(() => handoffTo('20') !== null);
		await until(() => handoffTo('40') !== null, 'the other series to move on');
		await until(() => resumeFor(SECOND) !== null, "the next show's matched card");
		expect(movedOn()).toBe(false);
		expect(resumeFor(FIRST)).not.toBeNull();
	});

	it('keeps the ordinary card when the next show was played from a row whose recorded entry Kitsu deleted', async () => {
		// The row recorded 99, which Kitsu no longer has; it is matched
		// again by its title, to the next show, and its card shows that.
		mountHome({ history: [row('2', 'p-second', '99', SECOND), row('13'), controlRow()] });
		const movedOn = everHeld(() => handoffTo('20') !== null);
		await until(() => handoffTo('40') !== null, 'the other series to move on');
		await until(() => resumeFor(SECOND) !== null, "the next show's matched card");
		await new Promise((r) => setTimeout(r, 50));
		expect(movedOn()).toBe(false);
		expect(resumeFor(FIRST)).not.toBeNull();
	});

	it("keeps the ordinary card when the next show's page would resume from a row", async () => {
		// Home matched the row to another show, but the backend's resume
		// lookup, which the next show's own page goes by, names it for
		// the next show (its stored mapping, which Home passed over).
		const other = row('4', 'p-other', '77', OTHER);
		mountHome({ history: [row('13'), controlRow()], named: { '20': other } });
		const movedOn = everHeld(() => handoffTo('20') !== null);
		await until(() => handoffTo('40') !== null, 'the other series to move on');
		await new Promise((r) => setTimeout(r, 50));
		expect(movedOn()).toBe(false);
		expect(resumeFor(FIRST)).not.toBeNull();
	});

	it("moves on once the row the next show's page would resume from is removed", async () => {
		const other = row('4', 'p-other', '77', OTHER);
		mountHome({ history: [row('13'), other, controlRow()], named: { '20': other } });
		await until(() => handoffTo('40') !== null, 'the other series to move on');
		await until(() => resumeFor(OTHER) !== null, "the other row's card");
		expect(handoffTo('20')).toBeNull();
		await removeCard(OTHER);
		await until(() => handoffTo('20') !== null, 'the card to move on once the row is gone');
	});

	it('keeps a moved-on card in place while another row is removed', async () => {
		const other = row('4', 'p-other', '77', OTHER);
		mountHome({ history: [row('13'), other, controlRow()] });
		await until(
			() => handoffTo('20') !== null && handoffTo('40') !== null,
			'both cards to move on'
		);
		const leftAlone = everHeld(() => handoffTo('20') === null);
		await removeCard(OTHER);
		await until(() => resumeFor(OTHER) === null, 'the removed card to go');
		await new Promise((r) => setTimeout(r, 100));
		expect(leftAlone()).toBe(false);
		expect(handoffTo('20')).not.toBeNull();
	});

	it('keeps the ordinary card when the resume lookup cannot be read', async () => {
		const asked = mountHome({ history: [row('13'), controlRow()], namedFails: true });
		const movedOn = everHeld(() => handoffTo('20') !== null || handoffTo('40') !== null);
		await until(() => asked.includes('10') && asked.includes('30'), 'both series reads');
		await new Promise((r) => setTimeout(r, 100));
		expect(movedOn()).toBe(false);
		expect(resumeFor(FIRST)).not.toBeNull();
	});

	it('keeps the ordinary card when the series was read only in part', async () => {
		const asked = mountHome({
			history: [row('13'), controlRow()],
			franchise: () => ({ ...lineFrom10, complete: false })
		});
		const movedOn = everHeld(() => handoffTo('20') !== null);
		await settled(asked);
		expect(movedOn()).toBe(false);
		expect(resumeFor(FIRST)).not.toBeNull();
	});

	it("offers the source's episode past Kitsu's count rather than moving on", async () => {
		mountHome({ history: [row('13')], probedCap: 14 });
		await until(
			() =>
				resumeFor(FIRST)?.textContent?.includes(m.home_resume_episode_label({ episode: 14 })) ??
				false,
			'the card to offer episode 14'
		);
		await new Promise((r) => setTimeout(r, 50));
		expect(handoffTo('20')).toBeNull();
	});

	it('moves on in the default order when the settings cannot be read', async () => {
		mountHome({
			history: [row('13')],
			settings: () => new HttpResponse(null, { status: 500 })
		});
		await until(() => handoffTo('20') !== null, 'the card to move on as linked');
	});
});

describe('Continue Watching hand-off — leaving Home', () => {
	it('cancels a hand-off read still out, so a slow franchise walk holds no connection', async () => {
		const fetches = watchPageFetches();
		let release: (() => void) | null = null;
		// Kitsu is slow: the first show's hand-off read stays open while
		// the backend walks its series (and then asks its resume lookup,
		// which is the backend's own read now, inside this one).
		mountHome({
			history: [row('13')],
			franchise: () =>
				new Promise<Walk>((resolve) => {
					release = () => resolve('fail');
				})
		});
		const handoffRead = (f: { path: string; method: string }) =>
			f.method === 'POST' && f.path === '/api/watch-order/10/handoff';
		await until(() => fetches.sent.some(handoffRead), "the first show's hand-off read");
		unmount(app!);
		app = null;
		try {
			await until(() => fetches.failed.some(handoffRead), 'the hand-off read cancelled', 2000);
		} finally {
			(release as (() => void) | null)?.();
		}
	});

	it('cancels a rail read still out', async () => {
		// The resume lookup this case used to hold is read by the
		// backend inside the hand-off read now (the case above); what
		// Home itself still has out besides is its rail read.
		const fetches = watchPageFetches();
		const holding: Array<() => void> = [];
		mountHome({ history: [row('13')] });
		server.use(
			http.post(
				`${API_BASE}/api/watch-order/rail`,
				() =>
					new Promise<Response>((resolve) =>
						holding.push(() => resolve(new HttpResponse(null, { status: 503 })))
					)
			)
		);
		const railRead = (f: { path: string; method: string }) =>
			f.method === 'POST' && f.path === '/api/watch-order/rail';
		await until(() => fetches.sent.some(railRead), 'the rail read');
		await fetches.settled('the reads besides the rail', { except: railRead });
		unmount(app!);
		app = null;
		try {
			// Every rail read Home sent has been cancelled: the ones a
			// later read superseded, and the last, by Home going.
			await until(
				() => fetches.failed.filter(railRead).length === fetches.sent.filter(railRead).length,
				'every rail read cancelled',
				2000
			);
		} finally {
			for (const end of holding) end();
		}
	});
});
