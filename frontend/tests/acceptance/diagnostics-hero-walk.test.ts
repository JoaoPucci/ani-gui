// Acceptance: the Debug page shows the home hero's last pick as the
// backend made it — the airing preference's value and where the
// launch's flags came from, then every trending show the pick read, in
// order, with its art, its status, whether the preference decided it,
// and what the pick did with it. Read-only. Before Home has picked, it
// says so; when the read fails, it says that.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { kitsuRef } from './home-handlers';

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
import { heroShow } from '../../src/lib/hero/rotation';
import { heroPick } from '../../src/test/hero-pick-fake';

import type { AppInfo, HeroWalk, HeroWalkView, HistoryEntry } from '../../src/lib/api';
import { appInfoAnswer, errorAnswer } from '../answers';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	localStorage.clear();
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
	localStorage.clear();
});

const section = () => target.querySelector('[data-testid="hero-walk"]') as HTMLElement | null;
const text = () => section()?.textContent ?? '';
const rows = () =>
	Array.from(section()?.querySelectorAll('[data-testid="hero-walk-step"]') ?? []).map((row) =>
		Array.from(row.querySelectorAll('td')).map((td) => td.textContent?.trim() ?? '')
	);

function cover(id: string, title: string, status: string) {
	return {
		...kitsuRef(id, title, 12),
		status,
		cover_image: { tiny: null, small: null, large: `https://c.example/${id}.jpg`, original: null }
	};
}

/** The walk the backend makes of three finished shows and one airing
 *  show, all with covers, with the preference on. */
const WALK: HeroWalk = heroPick(
	[
		cover('1', 'First Finished', 'finished'),
		cover('2', 'Second Finished', 'finished'),
		cover('3', 'Third Finished', 'finished'),
		cover('4', 'Airing Now', 'current')
	].map(heroShow),
	true
).walk;

function serveWalk(view: HeroWalkView) {
	server.use(
		http.get(`${API_BASE}/api/home/hero/walk`, () => HttpResponse.json<HeroWalkView>(view))
	);
}

describe('the Debug page shows the home hero pick', () => {
	it('shows the flag, where it came from, and every show the last pick read', async () => {
		serveWalk({ flag: { name: 'hero_airing_first', on: true, origin: 'file' }, walk: WALK });
		app = mount(DiagnosticsPage, { target });
		await vi.waitFor(() => expect(rows()).toHaveLength(4), { timeout: 3000 });
		expect(section()?.querySelector('h2')?.textContent).toBe(m.diagnostics_section_hero_walk());
		expect(text()).toContain(
			m.diagnostics_hero_walk_flag({
				name: 'hero_airing_first',
				state: m.diagnostics_hero_walk_flag_on()
			})
		);
		expect(text()).toContain(
			m.diagnostics_hero_walk_origin({ origin: m.diagnostics_hero_walk_origin_file() })
		);
		expect(text()).toContain(m.diagnostics_hero_walk_signed_note());
		expect(text()).toContain(
			m.diagnostics_hero_walk_summary({
				read: 4,
				entries: 4,
				preference: m.diagnostics_hero_walk_flag_on()
			})
		);
		const yes = m.diagnostics_hero_walk_yes();
		const no = m.diagnostics_hero_walk_no();
		const art = m.diagnostics_hero_walk_art_cover();
		const taken = (slot: number, reason: string) => m.diagnostics_hero_walk_taken({ slot, reason });
		expect(rows()).toEqual([
			['1', 'First Finished', art, 'finished', no, taken(2, m.diagnostics_hero_walk_taken_art())],
			['2', 'Second Finished', art, 'finished', no, taken(3, m.diagnostics_hero_walk_taken_art())],
			[
				'3',
				'Third Finished',
				art,
				'finished',
				yes,
				m.diagnostics_hero_walk_skipped({ reason: m.diagnostics_hero_walk_skipped_not_airing() })
			],
			[
				'4',
				'Airing Now',
				art,
				m.diagnostics_hero_walk_status_airing(),
				yes,
				taken(1, m.diagnostics_hero_walk_taken_airing_art())
			]
		]);
	});

	it('says no pick has been made yet, with the flag off at its default', async () => {
		serveWalk({ flag: { name: 'hero_airing_first', on: false, origin: 'defaults' }, walk: null });
		app = mount(DiagnosticsPage, { target });
		await vi.waitFor(() => expect(text()).toContain(m.diagnostics_hero_walk_none()), {
			timeout: 3000
		});
		expect(text()).toContain(
			m.diagnostics_hero_walk_flag({
				name: 'hero_airing_first',
				state: m.diagnostics_hero_walk_flag_off()
			})
		);
		expect(text()).toContain(
			m.diagnostics_hero_walk_origin({ origin: m.diagnostics_hero_walk_origin_defaults() })
		);
		expect(text()).not.toContain(m.diagnostics_hero_walk_signed_note());
		expect(rows()).toEqual([]);
	});

	it('says the pick could not be read when the backend does not answer', async () => {
		server.use(http.get(`${API_BASE}/api/home/hero/walk`, () => errorAnswer(500)));
		app = mount(DiagnosticsPage, { target });
		await vi.waitFor(() => expect(text()).toContain(m.diagnostics_hero_walk_failed()), {
			timeout: 3000
		});
		expect(rows()).toEqual([]);
	});
});
