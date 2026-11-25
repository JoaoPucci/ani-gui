// Acceptance: the Debug page lists every feature tip by key, with its
// tool, its window for this install as the backend answered it, and a
// Seen toggle that sets it either way and reads the list again. An
// on-demand tip has no Seen state to toggle. "Show it" asks for the tip
// the next time its place is on screen. When the read fails, it says so.

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
import { onboarding, __resetOnboardingForTests } from '../../src/lib/onboarding/tips.svelte';
import type {
	AppInfo,
	HistoryEntry,
	OnboardingDebugTip,
	OnboardingDebugView
} from '../../src/lib/api';
import { appInfoAnswer, errorAnswer } from '../answers';

const STRIP: OnboardingDebugTip = {
	id: 'watch-order',
	kind: 'single',
	surface: 'show',
	moment: 'arrive',
	since: '0.19.0',
	until: null,
	opens: '0.19.0',
	ends: '0.21.0',
	flag: null,
	flag_on: false,
	flag_on_since: null,
	on_demand: false,
	offered: true,
	seen: false,
	pending: true,
	places: ['watch-order-strip'],
	how_to: null
};
const GUIDE: OnboardingDebugTip = {
	...STRIP,
	id: 'a-guide',
	kind: 'sequence',
	surface: 'franchise',
	opens: null,
	ends: null,
	on_demand: true,
	offered: false,
	pending: false,
	places: ['one', 'two']
};

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let seenSet: Map<string, boolean>;
let puts: { id: string; seen: boolean }[];

function useTips(fail = false) {
	seenSet = new Map();
	puts = [];
	server.use(
		http.get(`${API_BASE}/api/onboarding/debug`, () => {
			if (fail) return errorAnswer(503);
			const tips = [STRIP, GUIDE].map((t) => {
				const seen = seenSet.get(t.id) ?? t.seen;
				return { ...t, seen, pending: t.offered && !seen };
			});
			return HttpResponse.json<OnboardingDebugView>({
				version: '0.19.0',
				installed_in: null,
				tips
			});
		}),
		http.put(`${API_BASE}/api/onboarding/debug/seen`, async ({ request }) => {
			const body = (await request.json()) as { id: string; seen: boolean };
			puts.push(body);
			seenSet.set(body.id, body.seen);
			return new HttpResponse(null, { status: 204 });
		})
	);
}

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	__resetOnboardingForTests();
	server.use(
		http.get(`${API_BASE}/api/app-info`, () =>
			HttpResponse.json<AppInfo>(
				appInfoAnswer({
					version: '0.18.1',
					history_path: '/home/u/.local/state/ani-gui/history',
					proxy_base_url: API_BASE,
					removed_legacy_paths: []
				})
			)
		),
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json<HistoryEntry[]>([]))
	);
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
});

async function until(predicate: () => boolean, what: string, timeoutMs = 5000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}`);
}

const section = () => target.querySelector('[data-testid="tip-keys"]') as HTMLElement | null;
const row = (id: string) =>
	section()?.querySelector<HTMLElement>(`[data-testid="tip-key"][data-id="${id}"]`) ?? null;
const seenBox = (id: string) =>
	row(id)?.querySelector<HTMLInputElement>('input[type="checkbox"]') ?? null;

describe('the Debug page’s feature tips', () => {
	it('lists every tip by key with its tool and window', async () => {
		useTips();
		app = mount(DiagnosticsPage, { target });
		await until(() => row('watch-order') !== null, 'the list');
		expect(row('watch-order')!.textContent).toContain(m.diagnostics_tips_kind_single());
		expect(row('watch-order')!.textContent).toContain(
			m.diagnostics_tips_offered({ opens: '0.19.0', ends: '0.21.0' })
		);
		expect(row('a-guide')!.textContent).toContain(m.diagnostics_tips_on_demand());
		expect(section()!.textContent).toContain(m.diagnostics_tips_updated());
		expect(seenBox('a-guide')!.disabled).toBe(true);
	});

	it('sets seen either way and reads the list again', async () => {
		useTips();
		app = mount(DiagnosticsPage, { target });
		await until(() => seenBox('watch-order') !== null, 'the toggle');
		seenBox('watch-order')!.click();
		await until(() => row('watch-order')?.dataset.pending === 'false', 'seen');
		seenBox('watch-order')!.click();
		await until(() => row('watch-order')?.dataset.pending === 'true', 'unseen');
		expect(puts).toEqual([
			{ id: 'watch-order', seen: true },
			{ id: 'watch-order', seen: false }
		]);
	});

	it('Show it asks for the tip the next time its place is on screen', async () => {
		useTips();
		app = mount(DiagnosticsPage, { target });
		await until(() => row('watch-order') !== null, 'the list');
		const show = [...row('watch-order')!.querySelectorAll('button')].find(
			(b) => b.textContent?.trim() === m.diagnostics_tips_show()
		)!;
		show.click();
		await until(
			() => row('watch-order')!.textContent!.includes(m.diagnostics_tips_waiting()),
			'armed'
		);
		expect(onboarding.armed?.id).toBe('watch-order');
		expect(onboarding.armed?.places).toEqual(['watch-order-strip']);
	});

	it('says when the tips cannot be read', async () => {
		useTips(true);
		app = mount(DiagnosticsPage, { target });
		await until(
			() => section()?.textContent?.includes(m.diagnostics_tips_failed()) ?? false,
			'the failure'
		);
	});
});
