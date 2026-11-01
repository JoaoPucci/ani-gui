// Acceptance: the notice that startup tidied after a removed provider.
//
// When a streaming provider leaves the app, the backend's boot sweep
// removes the history rows a newer provider's row has taken over from,
// rewrites how the rows it keeps store their episode, and drops the
// stored entries nothing reads any more. It reports that on
// `/api/app-info` (`retired_provider_sweep`), and the diagnostics page
// is the only place a user is told their history changed.
//
// The unit test for `retiredSweepView` pins the decision; this pins the
// delivery from the wire name through the page's `$derived` and
// `{#if}` to the rendered words, in every locale the app ships.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';

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

import DiagnosticsPage from '../../src/routes/diagnostics/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { m } from '../../src/lib/paraglide/messages';
import { locales, setLocale } from '../../src/lib/paraglide/runtime';

interface Report {
	superseded: string[];
	renumbered: string[];
	numbering_rows: number;
	cache_rows: number;
}

const CLEAN: Report = { superseded: [], renumbered: [], numbering_rows: 0, cache_rows: 0 };

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

function serve(report: Report) {
	server.use(
		http.get(`${API_BASE}/api/app-info`, () =>
			HttpResponse.json({
				version: '0.18.0',
				history_path: '/home/u/.local/state/ani-gui/history',
				proxy_base_url: API_BASE,
				removed_legacy_paths: [],
				retired_provider_sweep: report
			})
		),
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json([]))
	);
}

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	setLocale('en', { reload: false });
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

/** The app-info answer has been applied: its version is on screen,
 *  and the notice is derived from the same answer. */
const infoShown = () => (target.textContent ?? '').includes('0.18.0');

async function render(report: Report) {
	serve(report);
	app = mount(DiagnosticsPage, { target });
	await until(infoShown, 'the app-info answer to be applied');
	return target.textContent ?? '';
}

describe('diagnostics — the retired-provider notice', () => {
	// The English heading, to tell a page stuck in English from one
	// that rendered the locale asked for.
	const enHeading = m.diagnostics_section_retired_sweep();

	for (const locale of locales) {
		it(`names what a launch removed and rewrote, in plain words (${locale})`, async () => {
			setLocale(locale, { reload: false });
			const text = await render({
				superseded: ['Naruto Shippuden'],
				renumbered: ['Bleach: Thousand-Year Blood War - The Separation'],
				numbering_rows: 4,
				cache_rows: 271
			});

			expect(text).toContain(m.diagnostics_section_retired_sweep());
			if (locale !== 'en') expect(text).not.toContain(enHeading);
			expect(text).toContain(m.diagnostics_retired_sweep_superseded());
			expect(text).toContain('Naruto Shippuden');
			expect(text).toContain(m.diagnostics_retired_sweep_renumbered());
			expect(text).toContain('Bleach: Thousand-Year Blood War - The Separation');
			// The stored entries are counted together: sidecar and cache.
			expect(text).toContain(m.diagnostics_retired_sweep_stored({ count: 275 }));
		});
	}

	it('the catalogue has a heading of its own per locale', () => {
		const headings = new Set<string>();
		for (const locale of locales) {
			setLocale(locale, { reload: false });
			headings.add(m.diagnostics_section_retired_sweep());
		}
		expect(headings.size).toBe(locales.length);
	});

	it('says nothing on a launch that changed nothing', async () => {
		const text = await render(CLEAN);
		expect(text).not.toContain(m.diagnostics_section_retired_sweep());
	});

	it('shows only what a launch that failed part way did', async () => {
		// The removal went; the renumbering failed, so nothing was
		// rewritten; one numbering row went with the removed row.
		const text = await render({
			superseded: ['Naruto Shippuden'],
			renumbered: [],
			numbering_rows: 1,
			cache_rows: 0
		});

		expect(text).toContain(m.diagnostics_section_retired_sweep());
		expect(text).toContain('Naruto Shippuden');
		expect(text).not.toContain(m.diagnostics_retired_sweep_renumbered());
		expect(text).toContain(m.diagnostics_retired_sweep_stored({ count: 1 }));
	});
});
