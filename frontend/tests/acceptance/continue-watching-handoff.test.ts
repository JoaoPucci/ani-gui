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
}

/** Mounts Home over `history`, the first show's series answered by `franchise`. */
function mountHome(s: Scenario) {
	const asked: string[] = [];
	server.use(
		...homeHandlers({ history: s.history }, [
			http.get(`${API_BASE}/api/settings`, () =>
				HttpResponse.json({ ...appConfig(), watch_order: s.order ?? 'linked' })
			),
			http.get(`${API_BASE}/api/kitsu/anime/10`, () =>
				HttpResponse.json(kitsuRef('10', FIRST, 13))
			),
			http.get(`${API_BASE}/api/kitsu/anime/20`, () =>
				HttpResponse.json(kitsuRef('20', SECOND, 13))
			),
			http.get(`${API_BASE}/api/kitsu/franchise/:id`, ({ params }) => {
				asked.push(String(params.id));
				if (params.id !== '10') return HttpResponse.error();
				return s.franchise ? s.franchise() : HttpResponse.json(lineFrom10);
			})
		])
	);
	app = mount(HomePage, { target });
	return asked;
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
		const asked = mountHome({ history: [row('3', 'p-second', '20', SECOND), row('13')] });
		await until(
			() => resumeFor(FIRST) !== null && resumeFor(SECOND) !== null && asked.length > 0,
			'both cards and the series read'
		);
		await new Promise((r) => setTimeout(r, 50));
		expect(handoffTo('20')).toBeNull();
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
			history: [row('13')],
			franchise: () => new HttpResponse(null, { status: 503 })
		});
		await until(() => resumeFor(FIRST) !== null && asked.length > 0, 'the card and the read');
		await new Promise((r) => setTimeout(r, 50));
		expect(handoffTo('20')).toBeNull();
		expect(resumeFor(FIRST)).not.toBeNull();
	});

	it('keeps the ordinary card where the show leads two ways as linked', async () => {
		const asked = mountHome({
			history: [row('13')],
			franchise: () => HttpResponse.json(forkFrom10)
		});
		await until(() => resumeFor(FIRST) !== null && asked.length > 0, 'the card and the read');
		await new Promise((r) => setTimeout(r, 50));
		expect(handoffTo('20')).toBeNull();
		expect(handoffTo('25')).toBeNull();
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
});
