// Acceptance: Settings → Playback carries the "Watch order" choice,
// As linked (the default) or As released, as two buttons, and a
// choice persists `watch_order` through the settings endpoint.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { appConfig } from './home-handlers';
import { appInfoAnswer, malformedAnswer } from '../answers';
import { m } from '../../src/lib/paraglide/messages';

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

import SettingsPage from '../../src/routes/settings/+page.svelte';
import {
	__resetApiBaseForTests,
	type AppInfo,
	type Config,
	type ShowView
} from '../../src/lib/api';
import { rememberedAnswer, settledAnswer } from '../../src/lib/watch-order/session-memo';

const APP_INFO: AppInfo = appInfoAnswer({
	version: '0.12.1',
	history_path: '/home/u/.local/state/ani-gui/history',
	proxy_base_url: 'http://127.0.0.1:31337',
	removed_legacy_paths: []
});

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

/** The settings answer with `watchOrder` stored: an order the backend
 *  reads back (it reads any other as As linked), or one it never sends,
 *  for what the page does with it. */
function useSettings(watchOrder: Config['watch_order'] | 'story', putFails = false) {
	const puts: Record<string, unknown>[] = [];
	server.use(
		http.get(`${API_BASE}/api/settings`, () =>
			watchOrder === 'story'
				? malformedAnswer({ ...appConfig(), watch_order: watchOrder })
				: HttpResponse.json<Config>({ ...appConfig(), watch_order: watchOrder })
		),
		http.put(`${API_BASE}/api/settings`, async ({ request }) => {
			puts.push((await request.json()) as Record<string, unknown>);
			return new HttpResponse(null, { status: putFails ? 503 : 204 });
		}),
		http.get(`${API_BASE}/api/app-info`, () => HttpResponse.json<AppInfo>(APP_INFO))
	);
	return puts;
}

const group = () =>
	target.querySelector(`[role="group"][aria-label="${m.watchorder_setting_aria()}"]`);
const button = (label: string) =>
	[...(group()?.querySelectorAll('button') ?? [])].find((b) => b.textContent?.trim() === label) ??
	null;
const pressed = (label: string) => button(label)?.getAttribute('aria-pressed');

describe('settings — the watch order', () => {
	it('offers As linked, chosen by default, and persists As released', async () => {
		const puts = useSettings('linked');
		app = mount(SettingsPage, { target });
		await until(() => group() !== null, 'the watch-order choice');
		expect(target.textContent).toContain(m.watchorder_setting_key());
		expect(target.textContent).toContain(m.watchorder_setting_hint());
		expect(pressed(m.watchorder_setting_linked())).toBe('true');
		expect(pressed(m.watchorder_setting_released())).toBe('false');

		button(m.watchorder_setting_released())!.click();
		await until(() => puts.length > 0, 'the settings write');
		expect(puts.at(-1)!.watch_order).toBe('released');
		await until(() => pressed(m.watchorder_setting_released()) === 'true', 'the choice to show');
		expect(pressed(m.watchorder_setting_linked())).toBe('false');
	});

	it('shows a stored As released, and reads an unknown stored order as As linked', async () => {
		useSettings('released');
		app = mount(SettingsPage, { target });
		await until(() => group() !== null, 'the watch-order choice');
		expect(pressed(m.watchorder_setting_released())).toBe('true');
		unmount(app);
		target.replaceChildren();

		useSettings('story');
		app = mount(SettingsPage, { target });
		await until(() => group() !== null, 'the watch-order choice');
		expect(pressed(m.watchorder_setting_linked())).toBe('true');
	});
});

describe('settings — a change of the watch order', () => {
	// A show page visited earlier this session draws its remembered
	// watch-order answer in its first frame. The backend read that answer
	// in the order then in force, so once another order is saved, a show
	// reopened must not draw the old order, even for a frame.
	const shown = { view: { order: 'linked', strip: null, link: null } as ShowView };

	it('forgets the shows remembered in the old order once the new one is saved', async () => {
		const puts = useSettings('linked');
		settledAnswer('42', shown);
		app = mount(SettingsPage, { target });
		await until(() => group() !== null, 'the watch-order choice');
		button(m.watchorder_setting_released())!.click();
		await until(() => puts.length > 0, 'the settings write');
		await until(() => rememberedAnswer('42') === null, 'the remembered answers to go');
	});

	it('keeps them when the order saved is the one in force', async () => {
		const puts = useSettings('linked');
		settledAnswer('42', shown);
		app = mount(SettingsPage, { target });
		await until(() => group() !== null, 'the watch-order choice');
		button(m.watchorder_setting_linked())!.click();
		await until(() => puts.length > 0, 'the settings write');
		await new Promise((r) => setTimeout(r, 50));
		expect(rememberedAnswer('42')).toEqual(shown);
	});

	it('keeps them when the new order could not be saved', async () => {
		const puts = useSettings('linked', true);
		settledAnswer('42', shown);
		app = mount(SettingsPage, { target });
		await until(() => group() !== null, 'the watch-order choice');
		button(m.watchorder_setting_released())!.click();
		await until(() => puts.length > 0, 'the settings write');
		await new Promise((r) => setTimeout(r, 50));
		expect(rememberedAnswer('42')).toEqual(shown);
	});
});
