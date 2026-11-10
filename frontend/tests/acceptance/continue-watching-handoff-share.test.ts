// Acceptance: Home's hand-off walks leave the API lane room for the
// viewer.
//
// Each Continue row played through its finale asks for its series, and
// the franchise route holds its response open while the backend walks
// the franchise, several rounds of Kitsu reads when Kitsu is slow.
// Chromium opens at most six connections to the API's origin and
// queues the rest, and Home's own calls share them: a Continue card's
// resume reads its row again before it plays. With six finished rows
// walked at once, that read would wait behind walks the viewer never
// asked for. The walks take a share of the lane, and the resume goes
// through.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { appConfig, homeHandlers, kitsuRef } from './home-handlers';
import { sixConnections, type Connections } from './connections';
import { watchPageFetches } from './page-fetches';

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

const MIDDLE = 'Middle Show';

/** Six shows played through their finale, and one left part-way. */
const HISTORY = [
	...['101', '102', '103', '104', '105', '106'].map((k) => ({
		ep_no: '13',
		id: `p-${k}`,
		title: `Finished ${k}`,
		kitsu_id: k
	})),
	{ ep_no: '5', id: 'p-middle', title: MIDDLE, kitsu_id: '200' }
];

/** Series reads held open, ended when the case ends. */
const holding: Array<() => void> = [];

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let pool: Connections | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	localStorage.clear();
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	for (const end of holding.splice(0)) end();
	pool?.restore();
	pool = null;
	target.remove();
	localStorage.clear();
});

async function until(predicate: () => boolean, what: string, timeoutMs = 3000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

const resumeFor = (title: string) =>
	Array.from(target.querySelectorAll<HTMLButtonElement>('button.resume-card')).find((b) =>
		b.textContent?.includes(title)
	) ?? null;

describe('Continue Watching hand-off — the API lane', () => {
	it("leaves room for a card's resume while six series walks are slow", async () => {
		const plays: string[] = [];
		server.use(
			...homeHandlers({ history: HISTORY }, [
				http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
				http.post(`${API_BASE}/api/availability`, () =>
					HttpResponse.json({ available: true, episode_count: 13, approximate: false })
				),
				http.get(`${API_BASE}/api/kitsu/anime/:id`, ({ params }) => {
					const id = String(params.id);
					const row = HISTORY.find((h) => h.kitsu_id === id);
					return HttpResponse.json(kitsuRef(id, row?.title ?? id, 13));
				}),
				// Kitsu is slow: every series walk stays open.
				http.get(
					`${API_BASE}/api/kitsu/franchise/:id`,
					() =>
						new Promise<Response>((resolve) =>
							holding.push(() => resolve(new HttpResponse(null, { status: 503 })))
						)
				),
				http.post(`${API_BASE}/api/history/by-kitsu`, async ({ request }) => {
					const { ids } = (await request.json()) as { ids: string[] };
					return HttpResponse.json(Object.fromEntries(ids.map((k) => [k, null])));
				}),
				http.post(`${API_BASE}/api/play`, async ({ request }) => {
					const body = (await request.json()) as { title?: string };
					plays.push(body.title ?? '');
					return new HttpResponse(null, { status: 500 });
				})
			])
		);
		pool = sixConnections();
		const fetches = watchPageFetches();
		app = mount(HomePage, { target });
		const walks = () => fetches.sent.filter((f) => f.path.startsWith('/api/kitsu/franchise/'));
		await until(() => walks().length >= 2, 'the series walks');
		await until(() => resumeFor(MIDDLE) !== null, 'the part-way card');
		resumeFor(MIDDLE)!.click();
		// The resume reads its row again, then plays.
		await until(() => plays.length > 0, 'the play');
		// Two walks out, the other four waiting their turn.
		expect(walks()).toHaveLength(2);
	});
});
