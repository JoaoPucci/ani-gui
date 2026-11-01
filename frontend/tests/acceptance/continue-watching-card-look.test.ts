// Acceptance: a Continue Watching card lands on its artwork once.
//
// A card's artwork comes from two answers that arrive apart: the row's
// Kitsu match (the show's poster), then the episode details (the
// episode's thumbnail). The card is a skeleton until its final
// artwork is known — the thumbnail, the poster once the details have
// settled without one or have taken too long, or the two-letter
// placeholder — and then keeps it. The image itself is shown only once
// it has loaded and decoded. A return to Home draws a card it has
// already settled in its final look straight away.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { homeHandlers } from './home-handlers';

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
import { EPISODE_WAIT_CAP_MS, sessionLooks } from '../../src/lib/history/card-look';
import { m } from '../../src/lib/paraglide/messages';

const POSTER = 'https://media.kitsu.app/anime/1/poster/medium.jpg';
const THUMB = 'https://media.kitsu.app/episodes/4/thumb.jpg';

function show(id: string, title: string, poster: string | null = POSTER) {
	return {
		id,
		canonical_title: title,
		titles: {},
		abbreviated_titles: [],
		slug: title.toLowerCase().replace(/\s+/g, '-'),
		synopsis: null,
		poster_image: poster ? { medium: poster, large: null, small: null, original: null } : null,
		cover_image: null,
		episode_count: 26,
		status: 'finished',
		subtype: 'TV',
		start_date: '2019-01-01',
		average_rating: null
	};
}

function episode(number: number, thumb: string | null) {
	return {
		id: `ep-${number}`,
		canonical_title: `Episode title ${number}`,
		season_number: 1,
		number,
		relative_number: number,
		length: 24,
		synopsis: null,
		airdate: null,
		thumbnail: thumb ? { original: thumb } : null
	};
}

/** A response the scenario answers when it chooses. */
function held<T>() {
	let release: (v: T) => void = () => {};
	const answer = new Promise<T>((r) => (release = r));
	return { answer, release };
}

const ROW = { ep_no: '3', id: 'allanime-1', title: 'Cowboy Bebop' };
const OTHER = { ep_no: '1', id: 'allanime-2', title: 'Trigun' };

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	sessionLooks.clear();
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
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.innerHTML.slice(0, 4000)}`);
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const cell = (id = ROW.id) => target.querySelector<HTMLElement>(`[data-entry-id="${id}"]`);
const artwork = (id = ROW.id) => cell(id)?.querySelector<HTMLElement>('.resume-poster') ?? null;
const look = (id = ROW.id) => artwork(id)?.dataset.look ?? null;
const image = (id = ROW.id) => artwork(id)?.querySelector('img') ?? null;
const imageSource = (id = ROW.id) => {
	const src = image(id)?.getAttribute('src');
	return src ? new URL(src).searchParams.get('url') : null;
};
const resumeButton = (id = ROW.id) =>
	cell(id)?.querySelector<HTMLButtonElement>('button.resume-card') ?? null;

/** Every look a card has been seen in, in order, from the moment it drew. */
function watchLooks(id = ROW.id) {
	const seen: string[] = [];
	const record = () => {
		const l = look(id);
		if (l && seen[seen.length - 1] !== l) seen.push(l);
	};
	record();
	const observer = new MutationObserver(record);
	observer.observe(target, { subtree: true, childList: true, attributes: true });
	return {
		seen,
		stop: () => {
			record();
			observer.disconnect();
		}
	};
}

/** The handlers for one row whose search hit and episode page the scenario controls. */
function rowHandlers(
	opts: {
		rows?: (typeof ROW)[];
		search?: () => Promise<unknown> | unknown;
		episodes?: () => Promise<unknown> | unknown;
	} = {}
) {
	return homeHandlers({ history: opts.rows ?? [ROW] }, [
		http.post(`${API_BASE}/api/kitsu/search`, async ({ request }) => {
			const body = (await request.json()) as { query?: string };
			const hit = await (opts.search?.() ?? null);
			if (hit) return HttpResponse.json(hit);
			return HttpResponse.json(
				body.query?.includes('Trigun') ? [show('2', 'Trigun')] : [show('1', 'Cowboy Bebop')]
			);
		}),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, async () =>
			HttpResponse.json(await (opts.episodes?.() ?? [episode(4, THUMB)]))
		)
	]);
}

describe('Continue Watching card artwork', () => {
	it('is a skeleton, and not clickable, while the row is being matched', async () => {
		const search = held<unknown>();
		server.use(...rowHandlers({ search: () => search.answer }));
		app = mount(HomePage, { target });
		await until(() => cell() !== null, 'the card');
		expect(look()).toBe('pending');
		expect(resumeButton()).toBeNull();
		expect(target.textContent).not.toContain('CO');
		search.release([show('1', 'Cowboy Bebop')]);
		await until(() => resumeButton() !== null, 'the card to become clickable');
	});

	it('is clickable once matched while it waits for the episode, then lands on the thumbnail once', async () => {
		const episodes = held<unknown>();
		server.use(...rowHandlers({ episodes: () => episodes.answer }));
		app = mount(HomePage, { target });
		await until(() => resumeButton() !== null, 'the matched card');
		const looks = watchLooks();
		expect(look()).toBe('pending');
		expect(image()).toBeNull();
		expect(resumeButton()?.disabled).toBe(false);

		episodes.release([episode(4, THUMB)]);
		await until(() => look() === 'image', 'the card to settle');
		expect(imageSource()).toBe(THUMB);
		// Held over the skeleton until it has decoded.
		expect(image()?.dataset.revealed).toBeUndefined();
		image()?.dispatchEvent(new Event('load'));
		await until(() => image()?.dataset.revealed === 'fade', 'the thumbnail to be shown');
		expect(cell()?.textContent).toContain('Episode title 4');
		looks.stop();
		expect(looks.seen).toEqual(['pending', 'image']);
	});

	it('settles on the poster when the episode details are slow, and keeps it when they land', async () => {
		const episodes = held<unknown>();
		server.use(...rowHandlers({ episodes: () => episodes.answer }));
		app = mount(HomePage, { target });
		await until(() => resumeButton() !== null, 'the matched card');
		expect(look()).toBe('pending');
		await until(() => look() === 'image', 'the wait to run out', EPISODE_WAIT_CAP_MS + 2000);
		expect(imageSource()).toBe(POSTER);

		episodes.release([episode(4, THUMB)]);
		await until(() => cell()?.textContent?.includes('Episode title 4') === true, 'the details');
		await sleep(50);
		expect(imageSource()).toBe(POSTER);
	});

	it('settles on the poster when the episode has no thumbnail', async () => {
		server.use(...rowHandlers({ episodes: () => [episode(4, null)] }));
		app = mount(HomePage, { target });
		await until(() => look() === 'image', 'the card to settle');
		expect(imageSource()).toBe(POSTER);
	});

	it('settles on the poster when the episode details come back empty', async () => {
		server.use(...rowHandlers({ episodes: () => [] }));
		app = mount(HomePage, { target });
		await until(() => look() === 'image', 'the card to settle');
		expect(imageSource()).toBe(POSTER);
		expect(cell()?.textContent).toContain('Episode 4');
	});

	it('settles on the two-letter placeholder when there is no picture at all', async () => {
		server.use(
			...rowHandlers({ search: () => [show('1', 'Cowboy Bebop', null)], episodes: () => [] })
		);
		app = mount(HomePage, { target });
		await until(() => look() === 'placeholder', 'the card to settle');
		expect(artwork()?.textContent).toContain('CO');
	});

	it('settles on the placeholder when the row matches nothing', async () => {
		server.use(...rowHandlers({ search: () => [] }));
		app = mount(HomePage, { target });
		await until(() => look() === 'placeholder', 'the unmatched card');
		expect(cell()?.querySelector('a.resume-card')).not.toBeNull();
	});

	it('falls back to the poster, then the placeholder, when the artwork fails to load', async () => {
		server.use(...rowHandlers());
		app = mount(HomePage, { target });
		await until(() => imageSource() === THUMB, 'the thumbnail');
		image()?.dispatchEvent(new Event('error'));
		await until(() => imageSource() === POSTER, 'the poster');
		image()?.dispatchEvent(new Event('error'));
		await until(() => look() === 'placeholder', 'the placeholder');
	});

	it('draws a card in its final look at once on a return to Home', async () => {
		server.use(...rowHandlers());
		app = mount(HomePage, { target });
		await until(() => imageSource() === THUMB, 'the first visit to settle');
		await until(() => cell()?.textContent?.includes('Episode title 4') === true, 'its details');
		unmount(app);
		app = null;

		// The second visit's match is held: whatever the card draws
		// before it lands came from the first visit.
		const search = held<unknown>();
		server.use(...rowHandlers({ search: () => search.answer }));
		const looks = watchLooks();
		app = mount(HomePage, { target });
		await until(() => cell() !== null, 'the card');
		expect(look()).toBe('image');
		expect(imageSource()).toBe(THUMB);
		expect(cell()?.textContent).toContain('Episode title 4');
		expect(resumeButton()).toBeNull();

		search.release([show('1', 'Cowboy Bebop')]);
		await until(() => resumeButton() !== null, 'the card to become clickable');
		await sleep(50);
		looks.stop();
		expect(looks.seen).toEqual(['image']);
		expect(imageSource()).toBe(THUMB);
	});

	it('leaves the other cards loading normally when one is deleted mid-load', async () => {
		const episodes = held<unknown>();
		const deleted: string[] = [];
		server.use(
			http.delete(`${API_BASE}/api/history/:id`, ({ params }) => {
				deleted.push(String(params.id));
				return new HttpResponse(null, { status: 204 });
			}),
			...rowHandlers({ rows: [ROW, OTHER], episodes: () => episodes.answer })
		);
		app = mount(HomePage, { target });
		await until(
			() => resumeButton(ROW.id) !== null && resumeButton(OTHER.id) !== null,
			'both cards'
		);
		expect(look(ROW.id)).toBe('pending');

		cell(ROW.id)?.querySelector<HTMLButtonElement>('.resume-delete')?.click();
		const confirmButton = () =>
			Array.from(document.querySelectorAll('button')).find(
				(b) => b.textContent?.trim() === m.home_delete_confirm_button()
			) ?? null;
		await until(() => confirmButton() !== null, 'the confirm dialog');
		confirmButton()?.click();
		await until(() => deleted.includes(ROW.id), 'the delete');
		await until(() => cell(ROW.id) === null, 'the card to leave');

		episodes.release([episode(2, THUMB)]);
		await until(() => look(OTHER.id) === 'image', 'the remaining card to settle');
	});
});
