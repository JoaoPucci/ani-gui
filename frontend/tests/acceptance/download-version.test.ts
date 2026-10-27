// Acceptance: a download names the version remembered for the show,
// from either page that offers one, and names none when nothing is
// remembered.
//
// The choice is made on the detail page and kept in local storage per
// Kitsu id. The detail page's Download action and the play page's
// "More actions" entry both open the confirm dialog, and confirming
// opens the download stream: its query is what the backend resolves,
// so a download without the version would save the other copy of a
// show listed twice. A range is resolved from that same query, which
// is why the dialog's All and Range paths are covered as well as a
// single episode.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount, flushSync } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
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
import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests, type VersionInfo } from '../../src/lib/api';
import { applyChoice } from '../../src/lib/versions/version-choice';
import { downloadStore } from '../../src/lib/download/store.svelte';

const KITSU_ID = '7154';
const TITLE = "JoJo's Bizarre Adventure";
const EPISODES = 26;

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

afterEach(async () => {
	// Each case's download stays open — nothing ends its stream — and
	// holds a slot in the download queue, which outlives the case. Left
	// running, a later case's download would wait as queued behind the
	// earlier cases' instead of opening its stream.
	for (const item of [...downloadStore.items]) downloadStore.cancel(item.id);
	await new Promise((r) => setTimeout(r, 0));
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
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${document.body.textContent}`);
}

function kitsuEpisodes(count: number) {
	return Array.from({ length: count }, (_, i) => ({
		id: `ep-${i + 1}`,
		number: i + 1,
		relative_number: i + 1,
		canonical_title: `Episode ${i + 1}`,
		titles: {},
		synopsis: null,
		thumbnail: null,
		length_minutes: 24,
		air_date: '2019-01-01'
	}));
}

/** Every request either page makes for the show. */
function useShowHandlers() {
	let probes = 0;
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, EPISODES), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: EPISODES, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () =>
			HttpResponse.json(kitsuEpisodes(EPISODES))
		),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () => HttpResponse.json(null)),
		http.post(`${API_BASE}/api/availability`, () => {
			probes += 1;
			return HttpResponse.json({
				available: true,
				episode_count: EPISODES,
				extra_episodes: [],
				episode_count_approximate: false,
				versions: TWO_VERSIONS
			});
		}),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl'))
	);
	return { probes: () => probes };
}

/** The download streams confirming opened — not the page's resolves. */
const downloads = () =>
	FakeEventSource.instances
		.map((es) => new URL(es.url))
		.filter((u) => u.pathname === '/api/download/stream');

const dialog = () =>
	document.querySelector(
		'[role="dialog"][aria-labelledby="dl-confirm-title"]'
	) as HTMLElement | null;

/** A button in the confirm dialog, found by what it says. */
const dialogButton = (label: string) =>
	(Array.from(dialog()?.querySelectorAll('button') ?? []).find((b) =>
		(b.textContent ?? '').includes(label)
	) as HTMLButtonElement | undefined) ?? null;

const dialogInput = (ariaLabel: string) =>
	dialog()?.querySelector(`input[aria-label="${ariaLabel}"]`) as HTMLInputElement | null;

function typeInto(input: HTMLInputElement, value: string) {
	input.value = value;
	input.dispatchEvent(new Event('input', { bubbles: true }));
	flushSync();
}

/** Confirm the open dialog and return the download it opened. */
async function confirmDownload() {
	const before = downloads().length;
	await until(() => {
		const confirm = dialogButton(m.download_button_confirm());
		return confirm !== null && !confirm.disabled;
	}, 'the confirm button to enable');
	dialogButton(m.download_button_confirm())!.click();
	await until(() => downloads().length > before, 'the download stream');
	return downloads()[downloads().length - 1];
}

// ---------------------------------------------------------------- detail

/** The detail page's version control, found by its accessible name. */
const versionGroup = () =>
	target.querySelector(
		`[role="radiogroup"][aria-label="${m.detail_version_group_aria_label()}"]`
	) as HTMLElement | null;

const detailDownloadButton = () =>
	(Array.from(target.querySelectorAll('button')).find((b) =>
		(b.textContent ?? '').includes(m.detail_download_button())
	) as HTMLButtonElement | undefined) ?? null;

/** Mount the detail page and open the download dialog once the
 *  availability answer has set the range cap. */
async function openDetailDownload() {
	const h = useShowHandlers();
	app = mount(DetailPage, { target });
	await until(() => h.probes() > 0, 'the page-load availability lookup');
	await until(() => {
		const b = detailDownloadButton();
		return b !== null && !b.disabled;
	}, 'the Download action to enable');
	// The answer lands after the request; the cap it sets is read when
	// the dialog opens. The version choice it names shows it applied.
	await until(() => versionGroup() !== null, 'the availability answer to be applied');
	detailDownloadButton()!.click();
	await until(() => dialog() !== null, 'the download dialog');
}

async function chooseRange(from: string, to: string) {
	dialogButton(m.download_mode_range())!.click();
	flushSync();
	await until(() => dialogInput(m.download_range_from_aria_label()) !== null, 'the range inputs');
	typeInto(dialogInput(m.download_range_from_aria_label())!, from);
	typeInto(dialogInput(m.download_range_to_aria_label())!, to);
}

describe('detail route — a download names the remembered version', () => {
	it('names it on a download of the whole show', async () => {
		applyChoice(KITSU_ID, 'standard');
		await openDetailDownload();
		dialogButton(m.download_mode_all())!.click();
		flushSync();

		const dl = await confirmDownload();
		expect(dl.searchParams.get('episode')).toBe(`1-${EPISODES}`);
		expect(dl.searchParams.get('version')).toBe('standard');
	});

	it('names it on a download of one episode', async () => {
		applyChoice(KITSU_ID, 'standard');
		await openDetailDownload();
		await chooseRange('3', '3');

		const dl = await confirmDownload();
		expect(dl.searchParams.get('episode')).toBe('3');
		expect(dl.searchParams.get('version')).toBe('standard');
	});

	it('names it on a download of a range', async () => {
		applyChoice(KITSU_ID, 'standard');
		await openDetailDownload();
		await chooseRange('2', '5');

		const dl = await confirmDownload();
		expect(dl.searchParams.get('episode')).toBe('2-5');
		expect(dl.searchParams.get('version')).toBe('standard');
	});

	it('names no version when nothing is remembered', async () => {
		await openDetailDownload();
		dialogButton(m.download_mode_all())!.click();
		flushSync();

		const dl = await confirmDownload();
		expect(dl.searchParams.get('episode')).toBe(`1-${EPISODES}`);
		expect(dl.searchParams.has('version')).toBe(false);
	});
});

// ------------------------------------------------------------------ play

const moreButton = () =>
	target.querySelector(
		`button[aria-label="${m.play_more_aria_label()}"]`
	) as HTMLButtonElement | null;

const menuItem = (label: string) =>
	Array.from(target.querySelectorAll('button[role="menuitem"]')).find((b) =>
		(b.textContent ?? '').includes(label)
	) as HTMLButtonElement | undefined;

/** Mount the play page on a playing session for episode 3 and open
 *  the download dialog from "More actions". */
async function openPlayDownload() {
	useShowHandlers();
	setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '3', kind: 'mp4' });
	app = mount(PlayPage, { target });
	await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
	await until(() => moreButton() !== null, 'the More actions button under the player');
	// The entry refuses while the airing answer is still out; open the
	// menu until a click opens the dialog.
	await until(() => {
		if (dialog() !== null) return true;
		moreButton()!.click();
		flushSync();
		const item = menuItem(m.play_controls_menu_download());
		if (item && !item.disabled) {
			item.click();
			flushSync();
		}
		return dialog() !== null;
	}, 'the download dialog');
}

describe('play route — a download names the remembered version', () => {
	it('names it on a download of the playing episode', async () => {
		applyChoice(KITSU_ID, 'standard');
		await openPlayDownload();
		dialogButton(m.download_mode_this_episode())!.click();
		flushSync();

		const dl = await confirmDownload();
		expect(dl.searchParams.get('episode')).toBe('3');
		expect(dl.searchParams.get('version')).toBe('standard');
	});

	it('names it on a download of the whole show', async () => {
		applyChoice(KITSU_ID, 'standard');
		await openPlayDownload();
		dialogButton(m.download_mode_all())!.click();
		flushSync();

		const dl = await confirmDownload();
		expect(dl.searchParams.get('episode')).toBe(`1-${EPISODES}`);
		expect(dl.searchParams.get('version')).toBe('standard');
	});

	it('names no version when nothing is remembered', async () => {
		await openPlayDownload();
		dialogButton(m.download_mode_this_episode())!.click();
		flushSync();

		const dl = await confirmDownload();
		expect(dl.searchParams.get('episode')).toBe('3');
		expect(dl.searchParams.has('version')).toBe(false);
	});
});
