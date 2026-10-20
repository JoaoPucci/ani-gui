// Acceptance: the detail page offers a show's versions when the
// provider lists it twice, and the version chosen is the one played.
//
// hianime lists some seasons twice — "… (Uncensored)" beside a plain
// re-upload — and the availability answer names both as versions of
// one pick. The page offers the choice only then, with Uncensored
// selected unless another version was chosen before; choosing writes
// the choice down for the show, and every play request from then on
// names it. The choice outlives the page: mounting it again for the
// same show shows the version chosen.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount, flushSync } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
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
vi.mock('$lib/account/push-watched', () => ({
	syncWatchedToTrackers: vi.fn(async () => {})
}));

import DetailPage from '../../src/routes/anime/[id]/+page.svelte';
import { __resetApiBaseForTests, type VersionInfo } from '../../src/lib/api';

const KITSU_ID = '7154';
const TITLE = "JoJo's Bizarre Adventure";

const TWO_VERSIONS: VersionInfo[] = [
	{ kind: 'uncensored', show_key: 'hianime:jojos-bizarre-adventure-uncensored-1' },
	{ kind: 'standard', show_key: 'hianime:jojos-bizarre-adventure-2' }
];

type EsHandler = (ev: MessageEvent) => void;
class FakeEventSource {
	static instances: FakeEventSource[] = [];
	url: string;
	listeners: Record<string, EsHandler[]> = {};
	closed = false;
	constructor(url: string) {
		this.url = url;
		FakeEventSource.instances.push(this);
	}
	addEventListener(name: string, handler: EsHandler) {
		(this.listeners[name] ??= []).push(handler);
	}
	close() {
		this.closed = true;
	}
	dispatch(name: string, data?: string) {
		const ev = { data: data ?? '', type: name } as unknown as MessageEvent;
		for (const h of this.listeners[name] ?? []) h(ev);
	}
}
type GlobalLike = { EventSource?: typeof FakeEventSource };
const g = globalThis as unknown as GlobalLike;

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	window.localStorage.clear();
	setParams({ id: KITSU_ID });
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	delete g.EventSource;
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

type Posted = Record<string, unknown>;

/** Every request the page makes on mount; `versions` is what the
 *  availability answer names, `count` the episodes it says the provider
 *  carries, and mark-watched bodies are recorded. */
function useShowHandlers(versions: VersionInfo[] | undefined, watched: Posted[] = [], count = 26) {
	let probes = 0;
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, 26), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: 26, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () => HttpResponse.json(null)),
		http.post(`${API_BASE}/api/availability`, () => {
			probes += 1;
			return HttpResponse.json({
				available: true,
				episode_count: count,
				extra_episodes: [],
				episode_count_approximate: false,
				...(versions ? { versions } : {})
			});
		}),
		http.post(`${API_BASE}/api/play/mark-watched`, async ({ request }) => {
			watched.push((await request.json()) as Posted);
			return new HttpResponse(null, { status: 204 });
		})
	);
	return { probes: () => probes };
}

/** The version control, found by its accessible name. */
const versionGroup = () =>
	target.querySelector(
		`[role="radiogroup"][aria-label="${m.detail_version_group_aria_label()}"]`
	) as HTMLElement | null;

/** One version's radio, found by what it says. */
const versionRadio = (label: string) =>
	(Array.from(versionGroup()?.querySelectorAll('[role="radio"]') ?? []).find(
		(r) => (r.textContent ?? '').trim() === label
	) as HTMLElement | undefined) ?? null;

const checked = (label: string) => versionRadio(label)?.getAttribute('aria-checked');

/** The Play action, rendered once the page has a show to play. */
const playAction = () => target.querySelector('.actions button.btn') as HTMLButtonElement | null;

/** Resolution streams a click opened — not the page's warms. */
const clickStreams = () =>
	FakeEventSource.instances.filter((i) => !new URL(i.url).searchParams.has('prefetch'));

const versionOf = (es: FakeEventSource) => new URL(es.url).searchParams.get('version');

/** Whether episode `n`'s tile is cap-gated: the page has applied an
 *  availability answer whose count stops short of an aired episode. */
const capGated = (n: number) =>
	target.querySelector(`li[data-ep-num="${n}"] button`)?.getAttribute('title') ===
	m.detail_ep_recheck_idle();

/** How many episode tiles the strip shows. */
const tiles = () => target.querySelectorAll('li[data-ep-num]').length;

describe('detail route — the versions of a show listed twice', () => {
	it('offers no version choice when the show has one version', async () => {
		// The answer counts 11 episodes, one short of the 12 tiles the
		// strip's first page shows of Kitsu's 26: a strip of 11 shows the
		// page applied it, and the versions it names, none, with it.
		const h = useShowHandlers(undefined, [], 11);
		app = mount(DetailPage, { target });
		await until(() => h.probes() > 0, 'the page-load availability lookup');
		await until(() => playAction() !== null, 'the play action to render');
		await until(() => tiles() === 11, 'the availability answer to be applied');
		expect(versionGroup()).toBeNull();
		expect(target.textContent).not.toContain(m.detail_version_group_label());
	});

	it('offers no choice in a mode only one version carries', async () => {
		const h = useShowHandlers(
			[
				{ ...TWO_VERSIONS[0], has_mode: false },
				{ ...TWO_VERSIONS[1], has_mode: true }
			],
			[],
			11
		);
		app = mount(DetailPage, { target });
		await until(() => h.probes() > 0, 'the page-load availability lookup');
		await until(() => playAction() !== null, 'the play action to render');
		await until(() => tiles() === 11, 'the availability answer to be applied');
		expect(versionGroup()).toBeNull();
	});

	it('offers both versions with Uncensored selected when nothing was chosen', async () => {
		useShowHandlers(TWO_VERSIONS);
		app = mount(DetailPage, { target });
		await until(() => versionGroup() !== null, 'the version choice');

		expect(checked(m.detail_version_uncensored())).toBe('true');
		expect(checked(m.detail_version_standard())).toBe('false');
		// One tab stop for the group: the selected radio.
		expect(versionRadio(m.detail_version_uncensored())!.tabIndex).toBe(0);
		expect(versionRadio(m.detail_version_standard())!.tabIndex).toBe(-1);
	});

	it('plays the version chosen, and remembers it for the show', async () => {
		const watched: Posted[] = [];
		useShowHandlers(TWO_VERSIONS, watched);
		app = mount(DetailPage, { target });
		await until(() => versionGroup() !== null, 'the version choice');

		versionRadio(m.detail_version_standard())!.click();
		flushSync();
		expect(checked(m.detail_version_standard())).toBe('true');
		expect(checked(m.detail_version_uncensored())).toBe('false');

		await until(() => playAction() !== null, 'the play action');
		playAction()!.click();
		await until(() => clickStreams().length > 0, 'the click to resolve the episode');
		const click = clickStreams()[0];
		expect(versionOf(click)).toBe('standard');

		// The Continue Watching stamp names the same version, or it
		// would look up another row than the one the play wrote.
		click.dispatch(
			'done',
			JSON.stringify({
				session_id: 's-1',
				media_url: `${API_BASE}/s/s-1/file.mp4`,
				media_kind: 'mp4'
			})
		);
		await until(() => watched.length > 0, 'the Continue Watching stamp');
		expect(watched[0].version).toBe('standard');

		// A page mounted again for the show shows the version chosen.
		unmount(app);
		app = null;
		useShowHandlers(TWO_VERSIONS);
		app = mount(DetailPage, { target });
		await until(() => versionGroup() !== null, 'the version choice after a remount');
		expect(checked(m.detail_version_standard())).toBe('true');
		expect(checked(m.detail_version_uncensored())).toBe('false');
	});

	it('warms only the chosen version after a choice', async () => {
		useShowHandlers(TWO_VERSIONS);
		app = mount(DetailPage, { target });
		await until(() => versionGroup() !== null, 'the version choice');
		const before = FakeEventSource.instances.length;

		versionRadio(m.detail_version_standard())!.click();
		await until(() => FakeEventSource.instances.length > before, 'a warm for the chosen version');
		for (const es of FakeEventSource.instances.slice(before)) {
			expect(versionOf(es)).toBe('standard');
		}
	});

	it('moves the choice with the arrow keys, as a radio group does', async () => {
		useShowHandlers(TWO_VERSIONS);
		app = mount(DetailPage, { target });
		await until(() => versionGroup() !== null, 'the version choice');

		const uncensored = versionRadio(m.detail_version_uncensored())!;
		uncensored.focus();
		uncensored.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
		flushSync();
		expect(checked(m.detail_version_standard())).toBe('true');
		expect(document.activeElement).toBe(versionRadio(m.detail_version_standard()));
	});

	it('goes back to naming no version when Uncensored is chosen again', async () => {
		useShowHandlers(TWO_VERSIONS);
		app = mount(DetailPage, { target });
		await until(() => versionGroup() !== null, 'the version choice');

		versionRadio(m.detail_version_standard())!.click();
		flushSync();
		versionRadio(m.detail_version_uncensored())!.click();
		flushSync();
		expect(checked(m.detail_version_uncensored())).toBe('true');

		await until(() => playAction() !== null, 'the play action');
		playAction()!.click();
		await until(() => clickStreams().length > 0, 'the click to resolve the episode');
		expect(new URL(clickStreams()[0].url).searchParams.has('version')).toBe(false);
	});

	it('keeps playing what the storage holds when it refuses the choice', async () => {
		useShowHandlers(TWO_VERSIONS);
		// Only the choices' key refuses: the page keeps other records in
		// the same storage, and a storage refusing those too is a
		// different scenario from the one under test.
		// The stub replaces the method on the storage the page uses:
		// happy-dom's storage does not read it from Storage.prototype.
		const storage = window.localStorage;
		const write = storage.setItem.bind(storage);
		const setItem = vi.spyOn(storage, 'setItem').mockImplementation((key, value) => {
			if (key === 'ani-gui.version-choices') throw new Error('quota');
			write(key, value);
		});
		try {
			app = mount(DetailPage, { target });
			await until(() => versionGroup() !== null, 'the version choice');
			versionRadio(m.detail_version_standard())!.click();
			flushSync();
			// The play page reads the choice from storage: a choice the
			// storage never took is not one this page may play either.
			expect(checked(m.detail_version_uncensored())).toBe('true');
			await until(() => playAction() !== null, 'the play action');
			playAction()!.click();
			await until(() => clickStreams().length > 0, 'the click to resolve the episode');
			expect(new URL(clickStreams()[0].url).searchParams.has('version')).toBe(false);
		} finally {
			setItem.mockRestore();
		}
	});

	it('offers the versions a cap-gated recheck finds', async () => {
		// The page loaded on a cached row that named one version and a
		// count short of an aired episode; clicking that episode asks the
		// provider again past the cache, and the fresh row names two
		// versions. The choice appears without a reload, Uncensored
		// selected, since nothing was chosen for the show.
		const AIRED = 5;
		server.use(
			http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
			http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
				HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'current' })
			),
			http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
				HttpResponse.json({
					aired: AIRED,
					next_episode: AIRED + 1,
					next_airing_at: null,
					upcoming: []
				})
			),
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
			http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () => HttpResponse.json(null)),
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const body = (await request.json()) as { bypass_cache?: boolean };
				return HttpResponse.json(
					body.bypass_cache
						? {
								available: true,
								episode_count: AIRED + 2,
								extra_episodes: [],
								episode_count_approximate: false,
								versions: TWO_VERSIONS
							}
						: {
								available: true,
								episode_count: AIRED - 1,
								extra_episodes: [],
								episode_count_approximate: false
							}
				);
			}),
			http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 }))
		);
		app = mount(DetailPage, { target });
		const tile = () =>
			target.querySelector(`li[data-ep-num="${AIRED}"] button`) as HTMLButtonElement | null;
		await until(() => tile() !== null, `the tile for episode ${AIRED}`);
		await until(() => playAction() !== null, 'the play action to render');
		// The page-load answer's count gates the aired tile: it has been
		// applied, and the versions it names, none, with it.
		await until(() => capGated(AIRED), 'the page-load answer to be applied');
		expect(versionGroup()).toBeNull();

		tile()!.click();
		await until(() => versionGroup() !== null, 'the version choice the recheck found');
		expect(checked(m.detail_version_uncensored())).toBe('true');
	});
});
