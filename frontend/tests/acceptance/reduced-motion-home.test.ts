// Acceptance: under reduced motion, nothing on Home moves; with motion
// on, the same steps do move, so the scenarios cannot pass by watching
// nothing.
//
// The user asks the system for reduced motion and opens the app.
// The hero, the Continue Watching row — a card deleted from it
// included — the Watch Later rail and the rails' scroll arrows must
// all stand still. This tier sees the motion script starts (see
// ./motion.ts); the CSS half is asserted in a real Chromium by the
// Electron e2e.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { homeHandlers, kitsuRef } from './home-handlers';
import { recordMotion, setReducedMotion, type Motion } from './motion';

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
import { accountStore } from '../../src/lib/account/store.svelte';
import { m } from '../../src/lib/paraglide/messages';

const TRENDING = [
	kitsuRef('1', 'First Feature', 12),
	kitsuRef('2', 'Second Feature', 12),
	kitsuRef('3', 'Third Feature', 12)
];
const TOP_RATED = ['A', 'B', 'C', 'D', 'E'].map((t, i) => kitsuRef(String(20 + i), `Top ${t}`, 12));
const HISTORY = [
	{ ep_no: '3', id: 'allanime-1', title: 'Cowboy Bebop' },
	{ ep_no: '5', id: 'allanime-2', title: 'Trigun' },
	{ ep_no: '7', id: 'allanime-3', title: 'Planetes' }
];
const PLANNED = { malId: 31, title: 'Planned Show' };

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let motion: Motion;
let restoreLayout: () => void = () => {};

/** happy-dom lays nothing out: every strip reads 0 wide. Play the
 *  layout engine for them, so a rail of cards overflows its 500px
 *  viewport and offers its arrow. */
function installStripLayout(): () => void {
	const proto = HTMLElement.prototype;
	const saved = {
		clientWidth: Object.getOwnPropertyDescriptor(proto, 'clientWidth'),
		scrollWidth: Object.getOwnPropertyDescriptor(proto, 'scrollWidth')
	};
	const isStrip = (el: HTMLElement) => el.classList.contains('strip-scroll');
	Object.defineProperty(proto, 'clientWidth', {
		configurable: true,
		get(this: HTMLElement) {
			return isStrip(this) ? 500 : 0;
		}
	});
	Object.defineProperty(proto, 'scrollWidth', {
		configurable: true,
		get(this: HTMLElement) {
			if (!isStrip(this)) return 0;
			const rail = this.querySelector('.strip-rail');
			return Math.max(500, (rail?.children.length ?? 0) * 300);
		}
	});
	return () => {
		for (const [key, desc] of Object.entries(saved)) {
			if (desc) Object.defineProperty(proto, key, desc);
			else delete (proto as unknown as Record<string, unknown>)[key];
		}
	};
}

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	// The Watch Later rail loads only for a connected tracker; seed the
	// store as the layout's hydrate would.
	accountStore.setConnected('anilist', {
		access_token: 'test-bearer',
		refresh_token: null,
		expires_at_epoch_s: 4_000_000_000,
		user_id: 'user-1',
		username: 'tester',
		avatar_url: null
	});
	restoreLayout = installStripLayout();
	motion = recordMotion();
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	motion.stop();
	restoreLayout();
	accountStore.setDisconnected('anilist');
	vi.useRealTimers();
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

const byLabel = (label: string) =>
	Array.from(target.querySelectorAll('button')).find(
		(b) => b.getAttribute('aria-label') === label
	) ?? null;
const byText = (text: string) =>
	Array.from(target.querySelectorAll('button')).find((b) => b.textContent?.includes(text)) ?? null;
const heroText = () => target.querySelector('section.hero')?.textContent ?? '';
const continueCards = () => target.querySelectorAll('.resume-cell');

function mountHome(deleted: string[] = []) {
	server.use(
		...homeHandlers({ history: HISTORY }, [
			http.get(`${API_BASE}/api/kitsu/trending-anilist`, () => HttpResponse.json(TRENDING)),
			http.get(`${API_BASE}/api/kitsu/top-rated`, () => HttpResponse.json(TOP_RATED)),
			http.post(`${API_BASE}/api/kitsu/search`, async ({ request }) => {
				const { query } = (await request.json()) as { query: string };
				const row = HISTORY.find((h) => h.title === query);
				return HttpResponse.json(row ? [kitsuRef(row.id.slice(-1) + '00', row.title, 26)] : []);
			}),
			http.delete(`${API_BASE}/api/history/:id`, ({ params }) => {
				deleted.push(String(params.id));
				return new HttpResponse(null, { status: 204 });
			}),
			http.get(`${API_BASE}/api/account/list/anilist/cached`, () =>
				HttpResponse.json([
					{
						provider: 'anilist',
						media_id: PLANNED.malId,
						mal_id: PLANNED.malId,
						status: 'planning',
						progress_episodes: 0,
						score_0_to_100: null,
						updated_at_epoch_s: 1_700_000_000,
						title: PLANNED.title
					}
				])
			),
			http.post(`${API_BASE}/api/kitsu/by-mal-ids`, () =>
				HttpResponse.json([kitsuRef(String(PLANNED.malId), PLANNED.title, 12)])
			),
			http.post(`${API_BASE}/api/availability/batch`, () => HttpResponse.json({ cached: {} }))
		])
	);
	app = mount(HomePage, { target });
}

afterEach(() => setReducedMotion(false));

describe.each([
	['under reduced motion', true],
	['with motion on', false]
] as const)('Home %s', (_mode, reduced) => {
	beforeEach(() => setReducedMotion(reduced));

	it.runIf(reduced)(
		'lands the hero, the rows and the Watch Later rail without moving',
		async () => {
			mountHome();
			await until(() => heroText().includes('First Feature'), 'the hero');
			await until(() => continueCards().length === HISTORY.length, 'the Continue Watching cards');
			await until(() => (target.textContent ?? '').includes(PLANNED.title), 'the Watch Later rail');
			await until(() => (target.textContent ?? '').includes('Top E'), 'the Top Rated rail');

			expect(motion.animations).toEqual([]);
		}
	);

	it(
		reduced ? 'holds the hero on its first title' : 'rotates the hero, fading between titles',
		async () => {
			vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
			mountHome();
			await until(() => heroText().includes('First Feature'), 'the hero');

			// One rotation period and a little more.
			vi.advanceTimersByTime(6_500);
			await new Promise((r) => setTimeout(r, 50));

			if (reduced) {
				expect(heroText()).toContain('First Feature');
				expect(heroText()).not.toContain('Second Feature');
				expect(motion.animations).toEqual([]);
			} else {
				expect(heroText()).toContain('Second Feature');
				expect(motion.animations.length).toBeGreaterThan(0);
			}
		}
	);

	it('removes a deleted Continue Watching card', async () => {
		const deleted: string[] = [];
		mountHome(deleted);
		await until(
			() => byLabel(m.home_delete_card_aria_label({ title: 'Cowboy Bebop' })) !== null,
			'the delete chip on the first card'
		);
		await until(() => continueCards().length === HISTORY.length, 'every card');

		byLabel(m.home_delete_card_aria_label({ title: 'Cowboy Bebop' }))!.click();
		await until(() => byText(m.home_delete_confirm_button()) !== null, 'the confirmation');
		byText(m.home_delete_confirm_button())!.click();
		await until(() => deleted.includes('allanime-1'), 'the delete to reach the backend');
		await until(() => continueCards().length === HISTORY.length - 1, 'the card to go');
		// Past the window the delete's motion runs in.
		await new Promise((r) => setTimeout(r, 400));

		if (reduced) expect(motion.animations).toEqual([]);
		else expect(motion.animations.some((a) => a.includes('resume-cell'))).toBe(true);
	});

	it('moves a rail along its arrow', async () => {
		mountHome();
		await until(() => (target.textContent ?? '').includes('Top E'), 'the Top Rated rail');
		const next = () =>
			Array.from(target.querySelectorAll<HTMLButtonElement>('.strip-arrow-end.visible'))[0] ?? null;
		await until(() => next() !== null, 'a rail to offer its next arrow');

		next()!.click();

		expect(motion.glides).toEqual(reduced ? [] : [expect.stringContaining('scrollBy')]);
	});
});
