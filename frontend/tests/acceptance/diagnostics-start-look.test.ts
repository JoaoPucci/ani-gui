// Acceptance: the diagnostics page owns the switch between the
// player's two start looks while both are tried out: the episode
// still (the default) and the title card. A pick is kept per machine,
// and the page shows what is kept.

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
import { START_LOOK_STORAGE_KEY } from '../../src/lib/play/start-look';
import { m } from '../../src/lib/paraglide/messages';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

const g = globalThis as { localStorage?: unknown };
const storageDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	localStorage.clear();
	server.use(
		http.get(`${API_BASE}/api/app-info`, () =>
			HttpResponse.json({
				version: '0.15.1',
				history_path: '/home/u/.local/state/ani-gui/history',
				proxy_base_url: API_BASE,
				removed_legacy_paths: []
			})
		),
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json([]))
	);
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	if (storageDescriptor) Object.defineProperty(globalThis, 'localStorage', storageDescriptor);
	else delete g.localStorage;
	localStorage.clear();
});

async function settle() {
	for (let i = 0; i < 20; i++) await new Promise((r) => setTimeout(r, 5));
}

function radio(label: string): HTMLInputElement {
	const l = Array.from(target.querySelectorAll('label')).find((x) =>
		(x.textContent ?? '').includes(label)
	);
	expect(l, `a radio labelled ${label}`).toBeDefined();
	return l!.querySelector('input[type="radio"]') as HTMLInputElement;
}

describe('diagnostics — the player start look', () => {
	it('starts on the episode still, and keeps the title card once picked', async () => {
		app = mount(DiagnosticsPage, { target });
		await settle();
		expect(target.textContent).toContain(m.diagnostics_section_start_look());
		expect(radio(m.diagnostics_start_look_still()).checked).toBe(true);
		expect(radio(m.diagnostics_start_look_card()).checked).toBe(false);

		radio(m.diagnostics_start_look_card()).click();
		await settle();
		expect(localStorage.getItem(START_LOOK_STORAGE_KEY)).toBe('card');

		unmount(app);
		app = mount(DiagnosticsPage, { target });
		await settle();
		expect(radio(m.diagnostics_start_look_card()).checked).toBe(true);
	});
});
