// Acceptance: the notice that startup tidied the cache.
//
// Opening the cache at launch deletes the rows this version never
// reads — kept under an earlier version's keys, or past their lifetime
// — and reports the counts on `/api/app-info` (`cache_reclaim`). The
// unit test for `cacheReclaimView` pins the decision; this pins the
// delivery from the wire name to the rendered words, in every locale.

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

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

function serve(undeclared_rows: number, expired_rows: number) {
	server.use(
		http.get(`${API_BASE}/api/app-info`, () =>
			HttpResponse.json({
				version: '0.18.0',
				history_path: '/home/u/.local/state/ani-gui/history',
				proxy_base_url: API_BASE,
				removed_legacy_paths: [],
				cache_reclaim: { undeclared_rows, expired_rows }
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

const infoShown = () => (target.textContent ?? '').includes('0.18.0');

async function render(undeclared: number, expired: number) {
	serve(undeclared, expired);
	app = mount(DiagnosticsPage, { target });
	await until(infoShown, 'the app-info answer to be applied');
	return target.textContent ?? '';
}

describe('diagnostics — the cache tidied at startup', () => {
	// The English heading, to tell a page stuck in English from one
	// that rendered the locale asked for.
	const enHeading = m.diagnostics_section_cache_reclaim();

	for (const locale of locales) {
		it(`counts the old and the expired entries a launch removed (${locale})`, async () => {
			setLocale(locale, { reload: false });
			const text = await render(1156, 180);

			expect(text).toContain(m.diagnostics_section_cache_reclaim());
			expect(text).toContain(m.diagnostics_cache_reclaim_description());
			if (locale !== 'en') expect(text).not.toContain(enHeading);
			expect(text).toContain(m.diagnostics_cache_reclaim_old({ count: 1156 }));
			expect(text).toContain(m.diagnostics_cache_reclaim_expired({ count: 180 }));
		});
	}

	it('the catalogue has a heading of its own per locale', () => {
		const headings = new Set<string>();
		for (const locale of locales) {
			setLocale(locale, { reload: false });
			headings.add(m.diagnostics_section_cache_reclaim());
		}
		expect(headings.size).toBe(locales.length);
	});

	it('says nothing on a launch that removed nothing', async () => {
		const text = await render(0, 0);
		expect(text).not.toContain(m.diagnostics_section_cache_reclaim());
	});

	it('shows the notice when only one kind went', async () => {
		const text = await render(0, 12);
		expect(text).toContain(m.diagnostics_section_cache_reclaim());
		expect(text).toContain(m.diagnostics_cache_reclaim_expired({ count: 12 }));
		// A kind none of went is not listed at zero.
		expect(text).not.toContain(m.diagnostics_cache_reclaim_old({ count: 0 }));
	});
});
