// Acceptance: Settings offers one quality, the download's, beside the
// other download setting, and an older version's quality setting does
// not carry into it.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { appConfig } from './home-handlers';
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
import { __resetApiBaseForTests } from '../../src/lib/api';

const APP_INFO = {
	version: '0.17.0',
	history_path: '/home/u/.local/state/ani-gui/history',
	proxy_base_url: 'http://127.0.0.1:31337',
	removed_legacy_paths: []
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

/** Mount Settings over `settings`, recording what it writes. */
function mountSettings(settings: Record<string, unknown>) {
	const writes: Record<string, unknown>[] = [];
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json({ ...appConfig(), ...settings })),
		http.put(`${API_BASE}/api/settings`, async ({ request }) => {
			writes.push((await request.json()) as Record<string, unknown>);
			return new HttpResponse(null, { status: 204 });
		}),
		http.get(`${API_BASE}/api/app-info`, () => HttpResponse.json(APP_INFO)),
		http.get(`${API_BASE}/api/account/status`, () =>
			HttpResponse.json({ anilist: null, mal: null })
		)
	);
	app = mount(SettingsPage, { target });
	return { writes };
}

/** The settings fields, in page order, by the name each shows. */
const fieldNames = () =>
	Array.from(target.querySelectorAll('.field-key')).map((el) => (el.textContent ?? '').trim());

/** The field named `name`. */
const field = (name: string) =>
	Array.from(target.querySelectorAll('.field')).find(
		(el) => (el.querySelector('.field-key')?.textContent ?? '').trim() === name
	) as HTMLElement | undefined;

/** Every group of buttons that offers a quality. */
const qualityGroups = () =>
	Array.from(target.querySelectorAll('[role="group"]')).filter((g) =>
		Array.from(g.querySelectorAll('button')).some((b) => (b.textContent ?? '').trim() === '1080')
	);

const pressed = (el: HTMLElement | undefined) =>
	Array.from(el?.querySelectorAll('button[aria-pressed="true"]') ?? []).map((b) =>
		(b.textContent ?? '').trim()
	);

describe('settings — the download quality', () => {
	it('is the one quality offered, right before the download progress strip', async () => {
		mountSettings({ download_quality: 'best' });
		await until(
			() => field(m.settings_field_download_quality_key()) !== undefined,
			'the download quality field'
		);
		expect(qualityGroups()).toHaveLength(1);
		const names = fieldNames();
		const at = names.indexOf(m.settings_field_download_quality_key());
		expect(names[at + 1]).toBe(m.settings_field_download_bar_key());
		expect(field(m.settings_field_download_quality_key())?.textContent).toContain(
			m.settings_field_download_quality_hint()
		);
		expect(pressed(field(m.settings_field_download_quality_key()))).toEqual([m.app_quality_best()]);
	});

	it('a pick is written as the download quality, and no other quality is written', async () => {
		const { writes } = mountSettings({ download_quality: 'best' });
		await until(
			() => field(m.settings_field_download_quality_key()) !== undefined,
			'the download quality field'
		);
		const button = Array.from(
			field(m.settings_field_download_quality_key())!.querySelectorAll('button')
		).find((b) => (b.textContent ?? '').trim() === '720');
		button!.click();
		await until(() => writes.length > 0, 'the settings write');
		expect(writes.at(-1)!.download_quality).toBe('720');
		expect('quality' in writes.at(-1)!).toBe(false);
	});

	it('shows Best for settings an older version wrote, whatever their quality said', async () => {
		mountSettings({ quality: '480', download_quality: 'best' });
		await until(
			() => field(m.settings_field_download_quality_key()) !== undefined,
			'the download quality field'
		);
		expect(pressed(field(m.settings_field_download_quality_key()))).toEqual([m.app_quality_best()]);
	});

	it('the Playback section says it holds the defaults for watching and downloading', async () => {
		mountSettings({});
		await until(
			() => (target.textContent ?? '').includes(m.settings_section_playback_title()),
			'the Playback section'
		);
		// The English copy itself: the section now holds a download
		// setting, so "every stream" no longer describes it.
		expect(m.settings_section_playback_hint()).toBe('defaults for watching and downloading');
		expect(target.textContent).toContain('defaults for watching and downloading');
	});
});

describe('settings — the audio default', () => {
	it('says each show remembers the audio picked on its page', async () => {
		mountSettings({});
		await until(() => field(m.settings_field_audio_key()) !== undefined, 'the Audio field');
		expect(field(m.settings_field_audio_key())?.textContent).toContain(
			m.settings_field_audio_default_hint()
		);
	});
});
