// Acceptance: a Continue card whose finale has been played through moves
// on to what comes next in the series. The card takes the finished
// show's place, says "Next", names both shows, and opens the next
// show's page; it does not start playback. It moves on only to one
// aired show, in the order the Watch order setting chooses, that has
// no history row of its own; otherwise the row keeps its ordinary
// card. Only a row at such a finale asks for its series.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { appConfig, homeHandlers, kitsuRef } from './home-handlers';

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
import { __resetApiBaseForTests } from '../../src/lib/api';
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

const entry = (id: string, title: string, start: string, episodes: number) => ({
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

const lineFrom10 = {
	anchor: '10',
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
const forkFrom10 = {
	...lineFrom10,
	entries: [FIRST_E, SECOND_E, BRANCH_E],
	links: [
		['10', '20'],
		['10', '25']
	]
};

const lineFrom30 = {
	...lineFrom10,
	anchor: '30',
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

interface Scenario {
	history: ReturnType<typeof row>[];
	franchise?: () => Response;
	order?: string;
	/** The settings read's answer; the Watch order `order` by default. */
	settings?: () => Response;
	/** What the source's availability probe counts for every show. */
	probedCap?: number;
	/**
	 * The rows the backend's resume lookup names for an id, where it
	 * is not the row that recorded it (`history_claim::names`).
	 */
	named?: Record<string, ReturnType<typeof row>>;
	/** The resume lookup fails. */
	namedFails?: boolean;
}

/** Mounts Home over `history`, the first show's series answered by `franchise`. */
function mountHome(s: Scenario) {
	const asked: string[] = [];
	server.use(
		...homeHandlers({ history: s.history }, [
			http.get(`${API_BASE}/api/settings`, () =>
				s.settings
					? s.settings()
					: HttpResponse.json({ ...appConfig(), watch_order: s.order ?? 'linked' })
			),
			http.post(`${API_BASE}/api/availability`, () =>
				HttpResponse.json({
					available: true,
					episode_count: s.probedCap ?? 13,
					approximate: false
				})
			),
			http.get(`${API_BASE}/api/kitsu/anime/30`, () =>
				HttpResponse.json(kitsuRef('30', CONTROL, 13))
			),
			// A row that recorded no Kitsu id is matched by its title.
			http.post(`${API_BASE}/api/kitsu/search`, async ({ request }) => {
				const body = JSON.stringify(await request.json());
				return HttpResponse.json(body.includes('Second') ? [kitsuRef('20', SECOND, 13)] : []);
			}),
			http.get(`${API_BASE}/api/kitsu/anime/10`, () =>
				HttpResponse.json(kitsuRef('10', FIRST, 13))
			),
			http.get(`${API_BASE}/api/kitsu/anime/20`, () =>
				HttpResponse.json(kitsuRef('20', SECOND, 13))
			),
			// An entry Kitsu has deleted: a row that recorded it is
			// matched again by its title.
			http.get(`${API_BASE}/api/kitsu/anime/99`, () =>
				HttpResponse.json(
					{ kind: 'upstream', status: 404, key: 'error.network.upstream' },
					{ status: 502 }
				)
			),
			// The backend's resume lookup: the row each id resumes from.
			http.post(`${API_BASE}/api/history/by-kitsu`, async ({ request }) => {
				if (s.namedFails) return new HttpResponse(null, { status: 503 });
				const { ids } = (await request.json()) as { ids: string[] };
				return HttpResponse.json(
					Object.fromEntries(
						ids.map((k) => [k, s.named?.[k] ?? s.history.find((r) => r.kitsu_id === k) ?? null])
					)
				);
			}),
			http.get(`${API_BASE}/api/kitsu/franchise/:id`, ({ params }) => {
				asked.push(String(params.id));
				if (params.id === '30') return HttpResponse.json(lineFrom30);
				if (params.id !== '10') return HttpResponse.error();
				return s.franchise ? s.franchise() : HttpResponse.json(lineFrom10);
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
			franchise: () => new HttpResponse(null, { status: 503 })
		});
		const movedOn = everHeld(() => handoffTo('20') !== null);
		await settled(asked);
		expect(movedOn()).toBe(false);
		expect(resumeFor(FIRST)).not.toBeNull();
	});

	it('keeps the ordinary card where the show leads two ways as linked', async () => {
		const asked = mountHome({
			history: [row('13'), controlRow()],
			franchise: () => HttpResponse.json(forkFrom10)
		});
		const movedOn = everHeld(() => handoffTo('20') !== null || handoffTo('25') !== null);
		await settled(asked);
		expect(resumeFor(FIRST)).not.toBeNull();
		expect(movedOn()).toBe(false);
	});

	it('moves on to the next show by release date when the setting says so', async () => {
		mountHome({
			history: [row('13')],
			franchise: () => HttpResponse.json(forkFrom10),
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
		const other = row('4', 'p-other', '77', 'Something Else');
		mountHome({ history: [row('13'), controlRow()], named: { '20': other } });
		const movedOn = everHeld(() => handoffTo('20') !== null);
		await until(() => handoffTo('40') !== null, 'the other series to move on');
		await new Promise((r) => setTimeout(r, 50));
		expect(movedOn()).toBe(false);
		expect(resumeFor(FIRST)).not.toBeNull();
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
			franchise: () => HttpResponse.json({ ...lineFrom10, complete: false })
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
