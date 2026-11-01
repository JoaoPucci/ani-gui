// Acceptance: a Continue Watching card whose episode still AniList's
// pacing held back takes the still once its retry lands — the card
// stays the same card, and only its image changes, fading in.

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

const TICKET = 17;
const STILL = 'https://img1.ak.crunchyroll.com/bebop-4.jpg';

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

function episodes(withStill: boolean) {
	return [3, 4, 5].map((n) => ({
		id: `bebop-${n}`,
		number: n,
		relative_number: n,
		canonical_title: `Session ${n}`,
		season_number: 1,
		length: 24,
		synopsis: null,
		airdate: '1998-04-03',
		thumbnail: withStill && n === 4 ? { original: STILL } : null
	}));
}

const card = () =>
	Array.from(target.querySelectorAll<HTMLElement>('.resume-card')).find((b) =>
		b.textContent?.includes('Cowboy Bebop')
	) ?? null;
const cardImg = () => card()?.querySelector<HTMLImageElement>('.resume-poster img') ?? null;

describe('home Continue Watching — a held-back still', () => {
	it('lands on the same card, which only changes its image', async () => {
		let land!: () => void;
		const landed = new Promise<void>((r) => (land = r));
		let hasLanded = false;
		void landed.then(() => (hasLanded = true));
		let reads = 0;
		let readsAfter = 0;
		server.use(
			...homeHandlers({ history: [{ ep_no: '3', id: 'allanime-1', title: 'Cowboy Bebop' }] }, [
				http.post(`${API_BASE}/api/kitsu/search`, () =>
					HttpResponse.json([{ ...kitsuRef('1', 'Cowboy Bebop', 26), status: 'finished' }])
				),
				http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => {
					reads += 1;
					if (hasLanded) readsAfter += 1;
					return HttpResponse.json(
						episodes(hasLanded),
						hasLanded ? undefined : { headers: { 'x-ani-gui-refill': String(TICKET) } }
					);
				}),
				http.post(`${API_BASE}/api/refills/wait`, async () => {
					await landed;
					return HttpResponse.json([{ ticket: TICKET, landed: true }]);
				}),
				http.post(`${API_BASE}/api/refills/release`, () => new HttpResponse(null, { status: 204 }))
			])
		);
		app = mount(HomePage, { target });
		await vi.waitFor(() => expect(card()).not.toBeNull(), { timeout: 5000 });
		await vi.waitFor(() => expect(reads).toBeGreaterThan(0), { timeout: 5000 });
		const before = card();
		expect(cardImg()?.src ?? '').not.toContain(encodeURIComponent(STILL));

		land();
		await vi.waitFor(() => expect(cardImg()?.src ?? '').toContain(encodeURIComponent(STILL)), {
			timeout: 3000
		});
		// Fetched once more, not on and on.
		expect(readsAfter).toBe(1);
		expect(card()).toBe(before);
		expect(cardImg()?.classList.contains('img-fade')).toBe(true);
	});
});
