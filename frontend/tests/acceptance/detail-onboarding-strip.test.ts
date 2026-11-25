// Acceptance: the watch-order strip's feature tip on a show's page.
//
// After an update, the first show page whose strip settles on screen
// dims around the strip and a balloon explains it, with focus on "Got
// it". Got it, Escape, a press outside the strip (past the first
// moments) or a press on the strip itself close it, and the backend
// hears it was seen; it does not come back on the same visit. A
// first-ever install (the backend says the tip is not pending) never
// sees it. A press outside goes no further than the dim.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { answerResponse, type ScenarioInputs } from './watch-order-answers';
import { page, setParams, setRouteId } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { availabilityAnswer } from '../answers';
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
import OnboardHost from '../../src/lib/onboarding/OnboardHost.svelte';
import { onboarding, __resetOnboardingForTests } from '../../src/lib/onboarding/tips.svelte';
import { isModalOpen, openModal } from '../../src/lib/components/modal-layer';
import {
	__resetApiBaseForTests,
	type AiringStatus,
	type AvailabilityBatchResponse,
	type AvailabilityResponse,
	type Config,
	type Franchise,
	type FranchiseEntry,
	type HistoryRowByKitsu,
	type KitsuAnimeRef,
	type KitsuEpisode,
	type OnboardingView
} from '../../src/lib/api';

const KITSU_ID = '42';
// The strip suite's own franchise, so its recorded answer serves here.
const entry = (id: string, title: string, subtype: string, start: string): FranchiseEntry => ({
	id,
	title,
	subtype,
	start,
	episodes: 12,
	status: 'finished',
	poster: null,
	cover: null
});

// Link order: Origins → Saga → Second Arc → Final Arc. Origins is a
// prequel story released after Second Arc, so release order puts it
// between Second Arc and Final Arc.
const ORIGINS = entry('5', 'Saga: Origins', 'OVA', '2013-04-01');
const SAGA = entry('10', 'Saga', 'TV', '2010-01-01');
const SECOND = entry('42', 'Saga: Second Arc', 'TV', '2012-01-01');
const FINAL = entry('50', 'Saga: Final Arc', 'TV', '2014-01-01');

const FRANCHISE: Franchise = {
	anchor: KITSU_ID,
	name: 'Saga',
	name_from: '10',
	entries: [SAGA, SECOND, ORIGINS, FINAL],
	links: [
		['10', '42'],
		['42', '50'],
		['5', '10']
	],
	versions: [],
	films: [],
	sides: [],
	recaps: [],
	parents: {},
	complete: true
};

const tips = (pending: boolean): OnboardingView => ({
	version: '0.19.0',
	tips: [
		{
			id: 'watch-order',
			kind: 'single',
			surface: 'show',
			moment: 'arrive',
			since: '0.19.0',
			places: ['watch-order-strip'],
			how_to: null,
			pending
		}
	]
});

/** Every place the IntersectionObserver is asked about is on screen. */
class OnScreenIO {
	constructor(private readonly cb: IntersectionObserverCallback) {}
	observe(el: Element) {
		const entry = {
			target: el,
			boundingClientRect: { top: 200, bottom: 400, width: 800, height: 200 }
		} as unknown as IntersectionObserverEntry;
		queueMicrotask(() => this.cb([entry], this as unknown as IntersectionObserver));
	}
	unobserve() {}
	disconnect() {}
}
class MeasuredRO {
	constructor(private readonly cb: ResizeObserverCallback) {}
	observe() {
		queueMicrotask(() => this.cb([], this as unknown as ResizeObserver));
	}
	disconnect() {}
}

let target: HTMLElement;
let apps: ReturnType<typeof mount>[] = [];
let seen: string[];

function useShow(pending: boolean) {
	seen = [];
	server.use(
		http.get(`${API_BASE}/api/onboarding`, () => HttpResponse.json<OnboardingView>(tips(pending))),
		http.post(`${API_BASE}/api/onboarding/seen`, async ({ request }) => {
			seen.push(((await request.json()) as { id: string }).id);
			return new HttpResponse(null, { status: 204 });
		}),
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json<Config>(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json<KitsuAnimeRef>({
				...kitsuRef(KITSU_ID, 'Saga: Second Arc', 12),
				subtype: 'TV'
			})
		),
		http.post(`${API_BASE}/api/watch-order/${KITSU_ID}/show`, () => {
			const inputs: ScenarioInputs = {
				route: 'show',
				id: KITSU_ID,
				order: 'linked',
				franchise: FRANCHISE,
				resumes: {}
			};
			return answerResponse('detail-watch-order-strip', inputs);
		}),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json<AiringStatus>({
				aired: 12,
				next_episode: null,
				next_airing_at: null,
				upcoming: []
			})
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json<KitsuEpisode[]>([])),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json<KitsuAnimeRef[]>([])),
		http.post(`${API_BASE}/api/availability/batch`, () =>
			HttpResponse.json<AvailabilityBatchResponse>({ cached: {}, playable_episode_counts: {} })
		),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json<AvailabilityResponse>(
				availabilityAnswer({
					available: true,
					episode_count: 12,
					extra_episodes: [],
					episode_count_approximate: false
				})
			)
		),
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () =>
			HttpResponse.json<HistoryRowByKitsu | null>(null)
		)
	);
}

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	__resetOnboardingForTests();
	setParams({ id: KITSU_ID });
	setRouteId('/anime/[id]');
	vi.stubGlobal('IntersectionObserver', OnScreenIO);
	vi.stubGlobal('ResizeObserver', MeasuredRO);
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	for (const a of apps) unmount(a);
	apps = [];
	target.remove();
	vi.unstubAllGlobals();
	setRouteId(null);
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}`);
}

const balloon = () => document.querySelector<HTMLElement>('[data-testid="onboarding-balloon"]');
const scrimCatch = () =>
	document.querySelector<HTMLElement>('[data-testid="onboarding-scrim"] .catch');
const strip = () => target.querySelector<HTMLElement>('[data-onboard="watch-order-strip"]');
const gotIt = () =>
	[...(balloon()?.querySelectorAll('button') ?? [])].find(
		(b) => b.textContent?.trim() === m.onboarding_got_it()
	) ?? null;

function mountPage() {
	apps.push(mount(DetailPage, { target }));
	apps.push(mount(OnboardHost, { target }));
}

async function shown() {
	await until(() => strip() !== null, 'the strip');
	await until(() => balloon()?.style.visibility === 'visible', 'the balloon');
}

describe('detail route — the strip’s feature tip', () => {
	it('explains the strip after an update, with focus on Got it, and Got it closes it seen', async () => {
		useShow(true);
		mountPage();
		await shown();
		expect(balloon()!.textContent).toContain(m.onboarding_watch_order_1_title());
		expect(balloon()!.textContent).toContain(m.onboarding_watch_order_1_body());
		expect(balloon()!.textContent).toContain('0.19.0');
		expect(document.activeElement).toBe(gotIt());
		expect(isModalOpen()).toBe(true);
		// The strip itself stays within reach; the rest of the page does not.
		expect(strip()!.closest('[inert]')).toBeNull();
		gotIt()!.click();
		await until(() => seen.length === 1, 'the close sent');
		expect(seen).toEqual(['watch-order']);
		expect(balloon()).toBeNull();
		expect(isModalOpen()).toBe(false);
		// Once per visit: it does not come back.
		await new Promise((r) => setTimeout(r, 800));
		expect(balloon()).toBeNull();
	});

	it('Escape closes it seen', async () => {
		useShow(true);
		mountPage();
		await shown();
		document.activeElement!.dispatchEvent(
			new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })
		);
		await until(() => seen.length === 1, 'the close sent');
		expect(balloon()).toBeNull();
	});

	it('a press outside closes it seen and reaches nothing under the dim', async () => {
		useShow(true);
		mountPage();
		await shown();
		await new Promise((r) => setTimeout(r, 450));
		const under = vi.fn();
		document.addEventListener('pointerdown', under);
		const press = new PointerEvent('pointerdown', { bubbles: true, cancelable: true });
		scrimCatch()!.dispatchEvent(press);
		document.removeEventListener('pointerdown', under);
		expect(press.defaultPrevented).toBe(true);
		expect(under).not.toHaveBeenCalled();
		await until(() => seen.length === 1, 'the close sent');
		expect(balloon()).toBeNull();
	});

	it('a press on the strip itself works and closes it seen', async () => {
		useShow(true);
		mountPage();
		await shown();
		// Past the first moments: a press then was aimed before the tip.
		await new Promise((r) => setTimeout(r, 450));
		const link = strip()!.querySelector('a')!;
		link.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, cancelable: true }));
		await until(() => seen.length === 1, 'the close sent');
		expect(balloon()).toBeNull();
	});

	it('shows nothing when the tip is not pending, as on a first-ever install', async () => {
		useShow(false);
		mountPage();
		await until(() => strip() !== null, 'the strip');
		await new Promise((r) => setTimeout(r, 900));
		expect(balloon()).toBeNull();
		expect(isModalOpen()).toBe(false);
	});

	it('a tip asked for from the Debug page shows though seen, and marks nothing seen', async () => {
		useShow(false);
		onboarding.showNow({ ...tips(false).tips[0], pending: true });
		mountPage();
		await shown();
		expect(onboarding.armed).toBeNull();
		gotIt()!.click();
		await until(() => balloon() === null, 'closed');
		await new Promise((r) => setTimeout(r, 100));
		expect(seen).toEqual([]);
	});

	it('waits for a dialog already open when the page mounts, and shows once it closes', async () => {
		useShow(true);
		const dialog = document.createElement('div');
		dialog.innerHTML = '<button>ok</button>';
		document.body.appendChild(dialog);
		const held = openModal(dialog);
		try {
			mountPage();
			await until(() => strip() !== null, 'the strip');
			await new Promise((r) => setTimeout(r, 900));
			expect(balloon()).toBeNull();
		} finally {
			held.close();
			dialog.remove();
		}
		await until(() => balloon()?.style.visibility === 'visible', 'the balloon after the dialog');
	});

	it('a preview of a pending tip shows once on the visit, and marks nothing seen', async () => {
		useShow(true);
		onboarding.showNow(tips(true).tips[0]);
		mountPage();
		await shown();
		gotIt()!.click();
		await until(() => balloon() === null, 'closed');
		await new Promise((r) => setTimeout(r, 900));
		expect(balloon()).toBeNull();
		expect(seen).toEqual([]);
	});

	it('the entry point offers Show me how, which closes it and asks for the walkthrough where the feature lives', async () => {
		useShow(false);
		onboarding.showNow({
			id: 'franchise-entry',
			kind: 'entry',
			surface: 'show',
			moment: 'arrive',
			since: '0.19.0',
			places: ['watch-order-franchise'],
			how_to: {
				id: 'franchise-tour',
				surface: 'franchise',
				places: ['franchise-masthead', 'franchise-order', 'franchise-rows']
			},
			pending: true
		});
		mountPage();
		await until(() => balloon()?.style.visibility === 'visible', 'the entry balloon');
		expect(balloon()!.textContent).toContain(m.onboarding_franchise_entry_1_title());
		const buttons = [...balloon()!.querySelectorAll('button')].map((b) => b.textContent?.trim());
		expect(buttons).toEqual([m.onboarding_show_how(), m.onboarding_got_it()]);
		const link = target.querySelector<HTMLAnchorElement>('[data-onboard="watch-order-franchise"]')!;
		const pressed = vi.fn((e: Event) => e.preventDefault());
		link.addEventListener('click', pressed);
		[...balloon()!.querySelectorAll('button')]
			.find((b) => b.textContent?.trim() === m.onboarding_show_how())!
			.click();
		await until(() => balloon() === null, 'closed');
		expect(pressed).toHaveBeenCalledTimes(1);
		expect(onboarding.armed?.id).toBe('franchise-tour');
		expect(onboarding.armedAs).toBe('guide');
		expect(seen).toEqual([]);
	});
});
