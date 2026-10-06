// Acceptance: a Continue row that records the Kitsu id of the show the
// user played renders that show. The provider lists Seitokai ni mo Ana
// wa Aru! as "There Is Also a Hole in the Student Organization!", and
// matching that title back to Kitsu landed on Here is Greenwood; the row
// now carries the page's id, so the home page asks Kitsu for that entry
// and nothing else.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { homeHandlers, kitsuRef } from './home-handlers';

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

const ROW = {
	ep_no: '1',
	id: 'hianime:there-is-also-a-hole-in-the-student-organization-10497',
	title: 'There Is Also a Hole in the Student Organization!',
	kitsu_id: '49877'
};

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

describe('a Continue row that records the show played', () => {
	it('renders that show and asks for no match', async () => {
		const searches: string[] = [];
		const mappings: string[] = [];
		server.use(
			...homeHandlers({ history: [ROW] }, [
				http.get(`${API_BASE}/api/kitsu/anime/49877`, () =>
					HttpResponse.json(kitsuRef('49877', 'Seitokai ni mo Ana wa Aru!', 12))
				),
				http.post(`${API_BASE}/api/kitsu/search`, async ({ request }) => {
					searches.push(JSON.stringify(await request.json()));
					return HttpResponse.json([kitsuRef('1623', 'Here is Greenwood', 6)]);
				}),
				http.get(`${API_BASE}/api/allmanga-kitsu-map/:showId`, ({ params }) => {
					mappings.push(String(params.showId));
					return HttpResponse.json('1623');
				})
			])
		);
		app = mount(HomePage, { target });

		await until(
			() =>
				Array.from(target.querySelectorAll('button')).some((b) =>
					b.textContent?.includes('Seitokai ni mo Ana wa Aru!')
				),
			'the recorded show to render as a resumable card'
		);
		expect(target.textContent).not.toContain('Here is Greenwood');
		expect(searches).toEqual([]);
		expect(mappings).toEqual([]);
	});
});
