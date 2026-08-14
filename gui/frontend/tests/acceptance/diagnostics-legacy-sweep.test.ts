// Acceptance: the one-time notice that startup deleted a file.
//
// Versions before 0.12 kept their own copy of the shell script under
// the cache root. The backend now deletes it during boot and reports
// the path on `/api/app-info`; the diagnostics page is the only place
// a user is ever told this happened.
//
// The unit test for `legacySweepView` pins the decision but not the
// delivery. Everything between the JSON field and the rendered path —
// the wire name `removed_legacy_paths`, the page's `$derived`, the
// `{#if}` around the block — could break without it noticing, and the
// value is present on exactly one launch per install, so nobody would
// be around to see it fail. That whole chain only exists once the
// route, the API layer and a real response are running together.

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

const SWEPT = '/home/u/.cache/ani-gui/ani-cli';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

/** app-info as the backend serialises it, with the sweep result. */
function appInfo(removed: string[]) {
	return {
		version: '0.12.0',
		history_path: '/home/u/.local/state/ani-gui/history',
		proxy_base_url: API_BASE,
		removed_legacy_paths: removed
	};
}

function serve(removed: string[]) {
	server.use(
		http.get(`${API_BASE}/api/app-info`, () => HttpResponse.json(appInfo(removed))),
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
});

async function settle() {
	for (let i = 0; i < 20; i++) await new Promise((r) => setTimeout(r, 5));
}

describe('diagnostics — the boot sweep', () => {
	it('names the file the app deleted on the launch that deleted it', async () => {
		serve([SWEPT]);
		app = mount(DiagnosticsPage, { target });
		await settle();

		const text = target.textContent ?? '';
		expect(text, 'the section heading should be present').toContain(
			m.diagnostics_section_legacy_sweep()
		);
		// The path is the load-bearing part. "We cleaned something up"
		// without saying what is not a report.
		expect(text, 'the removed path should be named').toContain(SWEPT);
	});

	it('says nothing on every other launch', async () => {
		// The overwhelming majority of launches, including every install
		// that never ran a version which kept a copy. A section that is
		// always present and always empty trains people to ignore the
		// page.
		serve([]);
		app = mount(DiagnosticsPage, { target });
		await settle();

		expect(target.textContent ?? '').not.toContain(m.diagnostics_section_legacy_sweep());
	});
});
