// Acceptance: the home rows' header actions, from Home, in both row
// styles the user can switch between.
//
// What the maintainer could not find was the Watch Later reload and
// the Continue Watching kebab: icon-only, 16px, parked at the far end
// of the window from the row they act on. Each row's actions are now
// labelled buttons inside that row's own header, after its title and
// caption; the Watch Later line also says how fresh the rail is. The
// cases below run once per row style, since the two styles are the
// same markup arranged differently and must behave identically.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount, flushSync } from 'svelte';

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
import { accountStore } from '../../src/lib/account/store.svelte';
import type { ListEntry, Provider } from '../../src/lib/account/types';
import {
	invalidateWatchLater,
	markRefreshed,
	readLastRefreshed
} from '../../src/lib/account/watch-later-refresh';
import { listChanges } from '../../src/lib/account/list-changes';
import { ROW_PREFS_STORAGE_KEY, type RowStyle } from '../../src/lib/rows/prefs';
import { m } from '../../src/lib/paraglide/messages';

const MIN = 60_000;
const HOUR = 60 * MIN;

const HISTORY = [{ ep_no: '3', id: 'allanime-1', title: 'Cowboy Bebop' }];
const PLANNED = { malId: 21, title: 'Planned Show' };

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let historyCleared = 0;
let pulls: Provider[] = [];

function connect(provider: Provider) {
	accountStore.setConnected(provider, {
		access_token: `bearer-${provider}`,
		refresh_token: null,
		expires_at_epoch_s: 4_000_000_000,
		user_id: `user-${provider}`,
		username: 'tester',
		avatar_url: null
	});
}

function planned(provider: Provider): ListEntry {
	return {
		provider,
		media_id: PLANNED.malId,
		mal_id: PLANNED.malId,
		status: 'planning',
		progress_episodes: 0,
		score_0_to_100: null,
		updated_at_epoch_s: 1_700_000_000,
		title: PLANNED.title
	};
}

/** Everything the scenarios share; `pull` answers the tracker re-pull. */
function serve(pull: (provider: Provider) => Promise<Response> | Response) {
	server.use(
		...homeHandlers({ history: HISTORY }, [
			http.get(`${API_BASE}/api/account/list/:provider/cached`, ({ params }) =>
				HttpResponse.json([planned(params.provider as Provider)])
			),
			http.post(`${API_BASE}/api/account/list/:provider`, ({ params }) => {
				pulls.push(params.provider as Provider);
				return pull(params.provider as Provider);
			}),
			http.post(`${API_BASE}/api/kitsu/by-mal-ids`, () =>
				HttpResponse.json([kitsuRef(String(PLANNED.malId), PLANNED.title, 12)])
			),
			http.post(`${API_BASE}/api/availability/batch`, () => HttpResponse.json({ cached: {} })),
			http.delete(`${API_BASE}/api/history`, () => {
				historyCleared += 1;
				return new HttpResponse(null, { status: 204 });
			})
		])
	);
}

const ok = (provider: Provider) => HttpResponse.json([planned(provider)]);

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	localStorage.clear();
	historyCleared = 0;
	pulls = [];
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	accountStore.setDisconnected('anilist');
	accountStore.setDisconnected('mal');
	localStorage.clear();
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

/** The home row whose title is `title`. */
function row(title: string): HTMLElement | null {
	return (
		(Array.from(target.querySelectorAll('section.strip')).find((s) =>
			s.querySelector('.strip-header h2')?.textContent?.includes(title)
		) as HTMLElement | undefined) ?? null
	);
}
const header = (title: string) => row(title)?.querySelector('.strip-header') as HTMLElement;
/** A button in the row's own header, found by its visible text. */
function action(title: string, label: string): HTMLButtonElement | null {
	const h = row(title)?.querySelector('.strip-header');
	return (
		(Array.from(h?.querySelectorAll('button') ?? []).find(
			(b) => b.textContent?.trim() === label
		) as HTMLButtonElement | undefined) ?? null
	);
}
const status = () =>
	header(m.account_watch_later_title())?.querySelector('.strip-meta')?.textContent?.trim() ?? '';
const short = (n: number, unit: 'minute' | 'hour') =>
	new Intl.RelativeTimeFormat('en', { numeric: 'always', style: 'short' }).format(-n, unit);

const WL = () => m.account_watch_later_title();
const CW = () => m.home_strip_continue_eyebrow();
const REFRESH = () => m.account_watch_later_refresh_action();
const CLEAR = () => m.home_clear_history_action();

async function mountHome(style: RowStyle) {
	localStorage.setItem(ROW_PREFS_STORAGE_KEY, JSON.stringify({ style, controls: false }));
	app = mount(HomePage, { target });
	await until(() => action(WL(), REFRESH()) !== null, 'the Watch Later refresh action');
	await until(() => action(CW(), CLEAR()) !== null, 'the Continue Watching clear action');
}

for (const style of ['inline', 'band'] as const) {
	describe(`home row actions — ${style} rows`, () => {
		it('labels each action in its own row’s header, the name being the visible text', async () => {
			connect('anilist');
			markRefreshed('anilist', Date.now() - 12 * MIN);
			serve(ok);
			await mountHome(style);

			for (const [title, label] of [
				[WL(), REFRESH()],
				[CW(), CLEAR()]
			] as const) {
				const b = action(title, label)!;
				// The accessible name is the text the user reads: no
				// aria-label or title standing in for it.
				expect(b.hasAttribute('aria-label'), label).toBe(false);
				expect(b.hasAttribute('title'), label).toBe(false);
				// It is the row's header that holds it, beside the title.
				expect(b.closest('.strip-header'), label).toBe(header(title));
			}
			// Clearing is one action now, not a menu of one.
			const clear = action(CW(), CLEAR())!;
			expect(clear.hasAttribute('aria-haspopup')).toBe(false);
			expect(clear.hasAttribute('aria-expanded')).toBe(false);
			expect(target.querySelector('[role="menu"]')).toBeNull();
		});

		it('marks every home row with the chosen style', async () => {
			connect('anilist');
			markRefreshed('anilist', Date.now() - 12 * MIN);
			serve(ok);
			await mountHome(style);
			const surfaces = Array.from(target.querySelectorAll('section.strip')).map((s) =>
				s.getAttribute('data-surface')
			);
			expect(surfaces.length).toBeGreaterThan(0);
			expect(new Set(surfaces)).toEqual(new Set([style === 'band' ? 'band' : 'plain']));
		});

		it('takes keyboard focus on each action', async () => {
			connect('anilist');
			markRefreshed('anilist', Date.now() - 12 * MIN);
			serve(ok);
			await mountHome(style);
			for (const b of [action(WL(), REFRESH())!, action(CW(), CLEAR())!]) {
				expect(b.disabled).toBe(false);
				expect(b.tabIndex).toBeGreaterThanOrEqual(0);
				b.focus();
				expect(document.activeElement).toBe(b);
			}
		});

		it('says how long ago the rail was updated', async () => {
			connect('anilist');
			markRefreshed('anilist', Date.now() - 12 * MIN);
			serve(ok);
			await mountHome(style);
			expect(status()).toContain(m.account_watch_later_updated({ when: short(12, 'minute') }));
		});

		it('shows the oldest connected tracker’s time', async () => {
			connect('anilist');
			connect('mal');
			markRefreshed('anilist', Date.now() - 5 * MIN);
			markRefreshed('mal', Date.now() - 3 * HOUR);
			serve(ok);
			await mountHome(style);
			expect(status()).toContain(m.account_watch_later_updated({ when: short(3, 'hour') }));
		});

		it('refreshes on click: refreshing while it pulls, just now once it lands', async () => {
			connect('anilist');
			markRefreshed('anilist', Date.now() - 12 * MIN);
			let release: () => void = () => {};
			const held = new Promise<void>((r) => (release = r));
			serve(async (p) => {
				await held;
				return ok(p);
			});
			await mountHome(style);

			action(WL(), REFRESH())!.click();
			await until(
				() => status().includes(m.account_watch_later_refreshing()),
				'the refreshing state'
			);
			expect(pulls).toEqual(['anilist']);
			expect(action(WL(), REFRESH())!.disabled).toBe(true);

			release();
			await until(
				() => status().includes(m.account_watch_later_updated_now()),
				'the just-updated state'
			);
			expect(action(WL(), REFRESH())!.disabled).toBe(false);
			expect(readLastRefreshed('anilist')).not.toBeNull();
			expect(Date.now() - readLastRefreshed('anilist')!).toBeLessThan(MIN);
		});

		it('is not yet updated when a tracker has never been pulled and the pull fails', async () => {
			connect('anilist');
			serve(() => new HttpResponse(null, { status: 500 }));
			localStorage.setItem(ROW_PREFS_STORAGE_KEY, JSON.stringify({ style, controls: false }));
			app = mount(HomePage, { target });
			await until(
				() => status().includes(m.account_watch_later_not_updated()),
				'the not-yet-updated state'
			);
			expect(pulls).toEqual(['anilist']);
		});

		it('stops vouching for the rail when a tracker write settles while it is on screen', async () => {
			connect('anilist');
			markRefreshed('anilist', Date.now() - 12 * MIN);
			serve(ok);
			await mountHome(style);
			expect(status()).toContain(short(12, 'minute'));
			// A write lands on the tracker: the snapshot is due a pull again
			// (the write path invalidates it), and the row reads its list
			// again. The pull it is due has not happened, so the line must
			// not go on saying the rail was updated 12 minutes ago.
			invalidateWatchLater('anilist');
			listChanges.announce({
				kitsuId: String(PLANNED.malId),
				providers: ['anilist'],
				landed: ['anilist'],
				source: 'editor'
			});
			await until(
				() => status().includes(m.account_watch_later_not_updated()),
				'the line to stop vouching for the rail'
			);
		});

		it('ticks: the age moves on without a reload', async () => {
			// The clock is faked from before the mount, so the timer the
			// page sets for the next change is a fake one this case can
			// run; it still advances with real time, so the requests and
			// the polling below go on as usual.
			vi.useFakeTimers({ shouldAdvanceTime: true, toFake: ['Date', 'setTimeout', 'clearTimeout'] });
			connect('anilist');
			markRefreshed('anilist', Date.now() - 12 * MIN);
			serve(ok);
			await mountHome(style);
			expect(status()).toContain(short(12, 'minute'));
			vi.advanceTimersByTime(MIN);
			flushSync();
			await until(() => status().includes(short(13, 'minute')), 'the age to move on');
		});

		it('clears history after the confirm, from the row’s own action', async () => {
			connect('anilist');
			markRefreshed('anilist', Date.now() - 12 * MIN);
			serve(ok);
			await mountHome(style);

			action(CW(), CLEAR())!.click();
			await until(() => target.querySelector('[role="dialog"]') !== null, 'the confirm dialog');
			// Nothing goes before the user confirms.
			expect(historyCleared).toBe(0);
			const confirm = Array.from(target.querySelectorAll('[role="dialog"] button')).find(
				(b) => b.textContent?.trim() === m.home_clear_confirm_button()
			) as HTMLButtonElement;
			confirm.click();
			await until(() => row(CW()) === null, 'the Continue Watching row to go');
			expect(historyCleared).toBe(1);
		});
	});
}
