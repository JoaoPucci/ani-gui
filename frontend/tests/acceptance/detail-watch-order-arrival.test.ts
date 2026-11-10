// Acceptance: the show page's watch-order strip arrives without
// pushing the page.
//
// The franchise walk behind the strip can take a second or two on a
// first visit, and the strip used to land above the synopsis after the
// page was drawn, pushing everything down. The show's own relation
// listing (the walk's first read) already says what is coming, so the
// page waits for that "first hop" together with the show's details,
// for at most a moment past them, and while the walk runs it holds the
// room of what is coming: a quiet outline of the strip under the real
// "Watch order" label, or of the franchise link. When the walk
// answers, the outline gives way to the strip where it stood; when
// nothing comes, it fades and folds away, at once (with the scroll
// kept) where the reader has scrolled past it. A show visited again in
// the same session draws its strip with its first frame. Under reduced
// motion nothing animates.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { recordMotion, setReducedMotion, type Motion } from './motion';
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

import DetailPage from '../../src/routes/anime/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';

const KITSU_ID = '42';

const entry = (id: string, title: string, subtype: string, start: string) => ({
	id,
	title,
	subtype,
	start,
	episodes: 12,
	status: 'finished',
	poster: null,
	cover: null
});

const SAGA = entry('10', 'Saga', 'TV', '2010-01-01');
const SECOND = entry('42', 'Saga: Second Arc', 'TV', '2012-01-01');
const FINAL = entry('50', 'Saga: Final Arc', 'TV', '2014-01-01');
const FILM = entry('60', 'Saga: The Movie', 'movie', '2015-01-01');

const none = { versions: [], films: [], sides: [], recaps: [], parents: {}, complete: true };
/** A run of three: the strip. */
const RUN = {
	...none,
	anchor: KITSU_ID,
	entries: [SAGA, SECOND, FINAL],
	links: [
		['10', '42'],
		['42', '50']
	]
};
/** The show alone with a film beside it: the franchise link alone. */
const LINK_ONLY = { ...none, anchor: KITSU_ID, entries: [SECOND], links: [], films: [FILM] };
/** The show alone: nothing. */
const ALONE = { ...none, anchor: KITSU_ID, entries: [SECOND], links: [] };

/** An answer the scenario releases by hand. */
function gate() {
	let open!: () => void;
	const opened = new Promise<void>((r) => (open = r));
	return { open, opened };
}

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let motion: Motion;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	setParams({ id: KITSU_ID });
	motion = recordMotion();
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(async () => {
	if (app) unmount(app);
	app = null;
	target.remove();
	motion.stop();
	vi.restoreAllMocks();
	await setReducedMotion(false);
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 5));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

interface ShowOptions {
	/** The first hop's answer; held until `hopGate` opens, if given. */
	hop?: () => Response;
	hopGate?: Promise<void>;
	/** The walk's answer; held until `walkGate` opens, if given. */
	franchise?: () => Response;
	walkGate?: Promise<void>;
	/** The show's details and the settings, held until these open. */
	detailsGate?: Promise<void>;
	settingsGate?: Promise<void>;
}

function useShow(opts: ShowOptions = {}) {
	const calls = { hop: 0, franchise: 0, walked: 0 };
	server.use(
		http.get(`${API_BASE}/api/settings`, async () => {
			await opts.settingsGate;
			return HttpResponse.json({ ...appConfig(), watch_order: 'linked' });
		}),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, async () => {
			await opts.detailsGate;
			return HttpResponse.json({ ...kitsuRef(KITSU_ID, 'Saga: Second Arc', 12), subtype: 'TV' });
		}),
		http.get(`${API_BASE}/api/kitsu/franchise/${KITSU_ID}/first-hop`, async () => {
			calls.hop++;
			await opts.hopGate;
			return opts.hop ? opts.hop() : HttpResponse.json({ signal: 'strip' });
		}),
		http.get(`${API_BASE}/api/kitsu/franchise/${KITSU_ID}`, async () => {
			calls.franchise++;
			await opts.walkGate;
			calls.walked++;
			return opts.franchise ? opts.franchise() : HttpResponse.json(RUN);
		}),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: 12, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability/batch`, () => HttpResponse.json({ cached: {} })),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: 12,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl')),
		http.post(`${API_BASE}/api/history/by-kitsu`, async ({ request }) => {
			const { ids } = (await request.json()) as { ids: string[] };
			return HttpResponse.json(Object.fromEntries(ids.map((id) => [id, null])));
		}),
		http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json({}))
	);
	return calls;
}

const title = () => target.querySelector('h1.title')?.textContent ?? '';
/** The strip: the section headed "Watch order". */
const strip = (): HTMLElement | null =>
	[...target.querySelectorAll('section')].find(
		(s) => s.querySelector('h2 .eyebrow-key')?.textContent === m.watchorder_strip_eyebrow()
	) ?? null;
const franchiseLink = () => target.querySelector(`a[href="/anime/${KITSU_ID}/franchise"]`);
const held = () => target.querySelector<HTMLElement>('[data-testid="wo-reserve"]');
/** The motion recorded on elements of class `cls`, as `<element> <ms>ms`. */
const moved = (cls: string) =>
	motion.animations.filter((a) => a.split(' ')[0].split('.').includes(cls));
const settle = () => new Promise((r) => setTimeout(r, 450));

describe('detail route — the watch-order strip arriving', () => {
	it('holds the strip’s room from the first frame, then gives it to the strip', async () => {
		const walk = gate();
		useShow({ walkGate: walk.opened });
		app = mount(DetailPage, { target });
		await until(() => title() !== '', 'the page');
		// Drawn with the page, before the walk answers.
		expect(held()).not.toBeNull();
		expect(held()!.getAttribute('aria-hidden')).toBe('true');
		expect(held()!.textContent).toContain(m.watchorder_strip_eyebrow());
		expect(strip()).toBeNull();
		walk.open();
		await until(() => strip() !== null, 'the strip');
		expect(held()).toBeNull();
		await settle();
		// The strip fades in where the outline stood; nothing folds.
		expect(moved('watch-order').some((a) => a.endsWith(' 220ms'))).toBe(true);
		expect(moved('watch-order-held')).toEqual([]);
	});

	it('holds the link’s room for a show whose franchise link comes alone', async () => {
		const walk = gate();
		useShow({
			hop: () => HttpResponse.json({ signal: 'link' }),
			franchise: () => HttpResponse.json(LINK_ONLY),
			walkGate: walk.opened
		});
		app = mount(DetailPage, { target });
		await until(() => title() !== '', 'the page');
		expect(held()).not.toBeNull();
		// The link has no "Watch order" label, and neither has its outline.
		expect(held()!.textContent).not.toContain(m.watchorder_strip_eyebrow());
		walk.open();
		await until(() => franchiseLink() !== null, 'the franchise link');
		expect(held()).toBeNull();
		expect(strip()).toBeNull();
	});

	it('holds nothing when the show’s listing foretells nothing', async () => {
		const walk = gate();
		const calls = useShow({
			hop: () => HttpResponse.json({ signal: 'none' }),
			franchise: () => HttpResponse.json(ALONE),
			walkGate: walk.opened
		});
		app = mount(DetailPage, { target });
		await until(() => title() !== '', 'the page');
		expect(held()).toBeNull();
		walk.open();
		await until(() => calls.franchise > 0, 'the walk');
		await settle();
		expect(held()).toBeNull();
		expect(strip()).toBeNull();
		expect(franchiseLink()).toBeNull();
	});

	it('fades the room and folds it away when the walk fails', async () => {
		const walk = gate();
		useShow({
			franchise: () => new HttpResponse(null, { status: 503 }),
			walkGate: walk.opened
		});
		app = mount(DetailPage, { target });
		await until(() => title() !== '', 'the page');
		expect(held()).not.toBeNull();
		walk.open();
		await until(() => held() === null, 'the room to go');
		expect(strip()).toBeNull();
		expect(target.querySelector('[role="alert"]')).toBeNull();
		// A 100 ms fade, then a 260 ms fold.
		expect(moved('watch-order-held').some((a) => a.endsWith(' 260ms'))).toBe(true);
	});

	it('goes at once and keeps the scroll when the room is above the viewport', async () => {
		const walk = gate();
		useShow({
			franchise: () => new HttpResponse(null, { status: 503 }),
			walkGate: walk.opened
		});
		const scrollBy = vi.spyOn(window, 'scrollBy');
		app = mount(DetailPage, { target });
		await until(() => held() !== null, 'the room');
		// The reader has scrolled to the episodes: the room is 500px above.
		vi.spyOn(held()!, 'getBoundingClientRect').mockReturnValue({
			top: -500,
			bottom: -198,
			height: 302,
			left: 0,
			right: 1000,
			width: 1000,
			x: 0,
			y: -500,
			toJSON: () => ({})
		});
		walk.open();
		await until(() => held() === null, 'the room to go');
		await until(() => scrollBy.mock.calls.length > 0, 'the scroll kept');
		expect(scrollBy).toHaveBeenCalledWith(0, -302);
		expect(moved('watch-order-held')).toEqual([]);
	});

	it('waits for the first hop together with the show’s details', async () => {
		const hop = gate();
		const walk = gate();
		const calls = useShow({ hopGate: hop.opened, walkGate: walk.opened });
		app = mount(DetailPage, { target });
		try {
			await until(() => calls.hop > 0, 'the first hop asked');
			await new Promise((r) => setTimeout(r, 120));
			// The details have answered; the page waits a moment for the hop.
			expect(title()).toBe('');
			hop.open();
			await until(() => title() !== '', 'the page');
			expect(held()).not.toBeNull();
		} finally {
			walk.open();
		}
	});

	it('draws the page past a first hop that is slow, holding nothing', async () => {
		const hop = gate();
		const walk = gate();
		const calls = useShow({ hopGate: hop.opened, walkGate: walk.opened });
		app = mount(DetailPage, { target });
		try {
			await until(() => calls.hop > 0, 'the first hop asked');
			const asked = Date.now();
			await until(() => title() !== '', 'the page', 3000);
			// Drawn about 300 ms past the details, not held for the hop.
			expect(Date.now() - asked).toBeLessThan(1500);
			expect(held()).toBeNull();
			// Too late to hold anything.
			hop.open();
			await settle();
			expect(held()).toBeNull();
			walk.open();
			await until(() => strip() !== null, 'the strip, as before');
		} finally {
			hop.open();
			walk.open();
		}
	});

	it('draws the page without waiting for the first hop once the walk has answered', async () => {
		const hop = gate();
		const details = gate();
		const calls = useShow({ hopGate: hop.opened, detailsGate: details.opened });
		app = mount(DetailPage, { target });
		try {
			// The walk answers (from a warm cache) before the details do.
			await until(() => calls.walked > 0, 'the walk answered');
			// The stand-in has answered; a moment for the page to read it.
			await new Promise((r) => setTimeout(r, 20));
			const opened = Date.now();
			details.open();
			await until(() => title() !== '', 'the page');
			// Well inside the 300 ms the page would wait for a hop.
			expect(Date.now() - opened).toBeLessThan(200);
			expect(held()).toBeNull();
		} finally {
			hop.open();
		}
	});

	it('holds the strip’s room until the order setting is read, when the walk answered first', async () => {
		const settings = gate();
		const details = gate();
		const hop = gate();
		const calls = useShow({
			settingsGate: settings.opened,
			detailsGate: details.opened,
			hopGate: hop.opened
		});
		app = mount(DetailPage, { target });
		try {
			await until(() => calls.walked > 0 && calls.hop > 0, 'the walk answered, the hop asked');
			// The stand-in has answered; a moment for the page to read it.
			await new Promise((r) => setTimeout(r, 20));
			// The walk has answered; the hop answers just after the details.
			details.open();
			await new Promise((r) => setTimeout(r, 30));
			hop.open();
			await until(() => title() !== '', 'the page');
			// The strip cannot draw before the order setting: its room is held.
			expect(held()).not.toBeNull();
			settings.open();
			await until(() => strip() !== null && held() === null, 'the strip');
		} finally {
			settings.open();
			hop.open();
		}
	});

	it('draws the strip with the first frame of a show visited again', async () => {
		useShow();
		app = mount(DetailPage, { target });
		await until(() => strip() !== null, 'the strip, first visit');
		unmount(app);
		app = null;

		// The second visit's walk does not answer until the end.
		const walk = gate();
		const hop = gate();
		useShow({ walkGate: walk.opened, hopGate: hop.opened });
		app = mount(DetailPage, { target });
		try {
			await until(() => title() !== '', 'the page, second visit');
			expect(strip()).not.toBeNull();
			expect(held()).toBeNull();
		} finally {
			walk.open();
			hop.open();
		}
	});

	it('draws a show visited again with its strip even when the order setting is read last', async () => {
		useShow();
		app = mount(DetailPage, { target });
		await until(() => strip() !== null, 'the strip, first visit');
		unmount(app);
		app = null;

		const settings = gate();
		const walk = gate();
		useShow({ settingsGate: settings.opened, walkGate: walk.opened });
		app = mount(DetailPage, { target });
		try {
			await new Promise((r) => setTimeout(r, 100));
			settings.open();
			await until(() => title() !== '', 'the page, second visit');
			// The page waited for the setting, so the strip came with it.
			expect(strip()).not.toBeNull();
		} finally {
			settings.open();
			walk.open();
		}
	});

	it('moves nothing under reduced motion', async () => {
		await setReducedMotion(true);
		const walk = gate();
		useShow({ walkGate: walk.opened });
		app = mount(DetailPage, { target });
		await until(() => title() !== '', 'the page');
		expect(held()).not.toBeNull();
		walk.open();
		await until(() => strip() !== null && held() === null, 'the strip, swapped at once');
		await settle();
		expect(moved('watch-order')).toEqual([]);
		expect(moved('watch-order-held')).toEqual([]);
	});

	it('folds nothing away under reduced motion', async () => {
		await setReducedMotion(true);
		const walk = gate();
		useShow({
			franchise: () => new HttpResponse(null, { status: 503 }),
			walkGate: walk.opened
		});
		app = mount(DetailPage, { target });
		await until(() => title() !== '', 'the page');
		expect(held()).not.toBeNull();
		walk.open();
		await until(() => held() === null, 'the room to go');
		await settle();
		expect(moved('watch-order-held')).toEqual([]);
	});
});
