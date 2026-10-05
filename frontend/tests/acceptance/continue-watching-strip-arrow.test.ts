// Acceptance: the Continue Watching row stops offering a right arrow
// once its cards no longer overflow it.
//
// The row renders a card per history row and then collapses the rows
// that resolve to one show, so it can start out overflowing and settle
// on fewer cards than fill the viewport. The strip has to notice its
// content shrank even though its own box did not move.

import { describe, it, vi, beforeEach, afterEach } from 'vitest';
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

import HomePage from '../../src/routes/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';

function ref(id: string, title: string, episodeCount: number) {
	return {
		id,
		canonical_title: title,
		titles: {},
		abbreviated_titles: [],
		slug: title.toLowerCase().replace(/\s+/g, '-'),
		synopsis: null,
		poster_image: null,
		cover_image: null,
		episode_count: episodeCount,
		status: 'finished',
		start_date: '2019-01-01',
		average_rating: null
	};
}

function config(mode: 'sub' | 'dub') {
	return {
		locale: 'en',
		mode,
		quality: 'best',
		external_player: '',
		external_player_kind: 'mpv',
		external_player_custom_args: '',
		syncplay_binary: '',
		image_cache_cap_mb: 100,
		auto_play_next: false,
		download_bottom_bar_enabled: true,
		auto_skip_op: false,
		auto_skip_ed: false,
		use_custom_player_controls: true,
		auto_update_anicli: false,
		update_include_prereleases: false,
		primary_account: ''
	};
}

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

/** happy-dom lays nothing out, so every scroller reads 0 wide. The
 *  scenario plays the layout engine for the strips: a 500px viewport
 *  holding 300px cards, so two cards overflow it and one does not. */
const VIEWPORT_PX = 500;
const CARD_PX = 300;

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
			return isStrip(this) ? VIEWPORT_PX : 0;
		}
	});
	Object.defineProperty(proto, 'scrollWidth', {
		configurable: true,
		get(this: HTMLElement) {
			if (!isStrip(this)) return 0;
			const rail = this.querySelector('.strip-rail');
			return Math.max(VIEWPORT_PX, (rail?.children.length ?? 0) * CARD_PX);
		}
	});
	return () => {
		for (const [key, desc] of Object.entries(saved)) {
			if (desc) Object.defineProperty(proto, key, desc);
			else delete (proto as unknown as Record<string, unknown>)[key];
		}
	};
}

const cells = () => target.querySelectorAll('.resume-cell');
const rightArrowShown = () =>
	target.querySelector('.strip-arrow-end')?.classList.contains('visible') ?? false;

describe('home Continue Watching scroll arrows', () => {
	let restoreLayout: () => void = () => {};
	beforeEach(() => {
		restoreLayout = installStripLayout();
	});
	afterEach(() => {
		restoreLayout();
	});

	it('hides the right arrow once rows collapse into fewer cards than overflow the row', async () => {
		// Held so the unresolved cards are on screen long enough to be
		// seen; resolving at once would collapse them before the first
		// look and the arrow would never have had a reason to show.
		let releaseSearch: () => void = () => {};
		const searchHeld = new Promise<void>((resolve) => {
			releaseSearch = resolve;
		});

		server.use(
			http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(config('sub'))),
			// Two rows for one show — what a show watched through two
			// providers, or catalogue drift between resolves, leaves in
			// the history file. Both resolve to the same Kitsu entry, so
			// the row collapses them into one card once they resolve.
			http.get(`${API_BASE}/api/history`, () =>
				HttpResponse.json([
					{ ep_no: '3', id: 'allanime-1', title: 'Cowboy Bebop' },
					{ ep_no: '2', id: 'allanime-2', title: 'Cowboy Bebop' }
				])
			),
			http.post(`${API_BASE}/api/kitsu/search`, async () => {
				await searchHeld;
				return HttpResponse.json([ref('1', 'Cowboy Bebop', 26)]);
			}),
			http.post(`${API_BASE}/api/availability`, () =>
				HttpResponse.json({ available: true, episode_count: 26, approximate: false })
			),
			http.get(`${API_BASE}/api/kitsu/trending-anilist`, () => HttpResponse.json([])),
			http.get(`${API_BASE}/api/kitsu/top-rated`, () => HttpResponse.json([])),
			http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json({})),
			http.get(`${API_BASE}/api/allmanga-kitsu-map/:showId`, () => HttpResponse.json(null)),
			http.get(`${API_BASE}/api/title-match`, () => HttpResponse.json(null)),
			http.put(`${API_BASE}/api/title-match`, () => new HttpResponse(null, { status: 204 })),
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([]))
		);

		app = mount(HomePage, { target });

		// Two placeholder cards overflow the viewport: the arrow is
		// right to be there before the rows resolve.
		await until(() => cells().length === 2, 'both history rows to render a card');
		await until(rightArrowShown, 'the right arrow to offer the overflowing row');

		releaseSearch();
		await until(() => cells().length === 1, 'the two rows to collapse into one card');

		// One card fits. The scroller's own box never changed size, so
		// only the rail's change can take the arrow away.
		await until(() => !rightArrowShown(), 'the right arrow to hide once nothing overflows', 1000);
	});
});
