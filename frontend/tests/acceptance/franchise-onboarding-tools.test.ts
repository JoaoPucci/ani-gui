// Acceptance: the short sequence and the entry point, the tools 0.19.0
// ships before a feature uses them, run here as the Debug page's "Show
// it" and a feature's "Show me how" start them.
//
// - A walkthrough asked for on the franchise page says "How it works"
//   and names no version, counts its steps, and moves with Next and
//   Back to Done; Skip ends it; nothing is marked seen.
// - The entry point on the show page offers "Show me how" beside "Got
//   it": it closes the entry, asks for the walkthrough on the feature's
//   page, and presses the entry point.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setRouteId } from './page-state.svelte';
import { answerResponse, type ScenarioInputs } from './watch-order-answers';
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

import FranchisePage from '../../src/routes/anime/[id]/franchise/+page.svelte';
import OnboardHost from '../../src/lib/onboarding/OnboardHost.svelte';
import ShowMeHow from '../../src/lib/onboarding/ShowMeHow.svelte';
import { onboarding, __resetOnboardingForTests } from '../../src/lib/onboarding/tips.svelte';
import { isModalOpen } from '../../src/lib/components/modal-layer';
import {
	__resetApiBaseForTests,
	type Franchise,
	type FranchiseEntry,
	type OnboardingView
} from '../../src/lib/api';

const KITSU_ID = '42';

const entry = (
	id: string,
	title: string,
	subtype: string,
	start: string,
	episodes = 12
): FranchiseEntry => ({
	id,
	title,
	subtype,
	start,
	episodes,
	status: 'finished',
	poster: null,
	cover: null
});

// Link order: Origins → Saga → Second Arc → Final Arc. Origins came
// out after Second Arc, so release order puts it third.
const ORIGINS = entry('5', 'Saga: Origins (OVA)', 'OVA', '2013-04-01', 2);
const SAGA = entry('10', 'Saga', 'TV', '2010-01-01');
const SECOND = entry('42', 'Saga: Second Arc', 'TV', '2012-01-01');
const FINAL = entry('50', 'Saga: Final Arc', 'TV', '2014-01-01');
const REMAKE = entry('70', 'Saga (2020)', 'TV', '2020-01-01', 24);
const FILM = entry('60', 'Saga: The Movie', 'movie', '2015-01-01', 1);
const SIDE = entry('80', 'Saga: Beach Episode', 'special', '2012-08-01', 1);
const RECAP = entry('90', 'Saga: Recap', 'special', '2013-01-01', 1);

const FRANCHISE: Franchise = {
	anchor: KITSU_ID,
	// The walk names it after its first series by start.
	name: 'Saga',
	name_from: '10',
	entries: [ORIGINS, SAGA, SECOND, FINAL],
	links: [
		['10', '42'],
		['42', '50'],
		['5', '10']
	],
	versions: [REMAKE],
	films: [FILM],
	sides: [SIDE],
	recaps: [RECAP],
	parents: {},
	complete: true
};

const GUIDE = {
	id: 'franchise-tour',
	surface: 'franchise' as const,
	places: ['franchise-masthead', 'franchise-order', 'franchise-rows']
};

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
	unobserve() {}
	disconnect() {}
}

let target: HTMLElement;
let apps: ReturnType<typeof mount>[] = [];
let seen: string[];

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	__resetOnboardingForTests();
	setParams({ id: KITSU_ID });
	setRouteId('/anime/[id]/franchise');
	vi.stubGlobal('IntersectionObserver', OnScreenIO);
	vi.stubGlobal('ResizeObserver', MeasuredRO);
	seen = [];
	server.use(
		http.get(`${API_BASE}/api/onboarding`, () =>
			HttpResponse.json<OnboardingView>({ version: '0.19.0', tips: [] })
		),
		http.post(`${API_BASE}/api/onboarding/seen`, async ({ request }) => {
			seen.push(((await request.json()) as { id: string }).id);
			return new HttpResponse(null, { status: 204 });
		}),
		http.post(`${API_BASE}/api/watch-order/${KITSU_ID}/franchise`, () => {
			const inputs: ScenarioInputs = {
				route: 'franchise',
				id: KITSU_ID,
				order: 'linked',
				franchise: FRANCHISE,
				resumes: {}
			};
			return answerResponse('franchise-page', inputs);
		})
	);
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
const action = (label: string) =>
	[...(balloon()?.querySelectorAll('button') ?? [])].find((b) => b.textContent?.trim() === label) ??
	null;

async function shown() {
	await until(() => balloon()?.style.visibility === 'visible', 'the balloon');
}

describe('the short sequence, as a walkthrough asked for', () => {
	it('says how it works, counts its steps, and moves with Next and Back to Done', async () => {
		onboarding.showGuide(GUIDE);
		apps.push(mount(FranchisePage, { target }));
		apps.push(mount(OnboardHost, { target }));
		await shown();
		expect(balloon()!.textContent).toContain(m.onboarding_guide());
		expect(balloon()!.textContent).not.toContain('0.19.0');
		expect(balloon()!.textContent).toContain(m.onboarding_franchise_tour_1_title());
		expect(balloon()!.textContent).toContain(m.onboarding_step_aria({ n: '1', total: '3' }));
		expect(action(m.onboarding_back())).toBeNull();
		action(m.onboarding_next())!.click();
		await until(
			() => balloon()?.textContent?.includes(m.onboarding_franchise_tour_2_title()) ?? false,
			'step 2'
		);
		expect(document.activeElement).toBe(action(m.onboarding_next()));
		action(m.onboarding_back())!.click();
		await until(
			() => balloon()?.textContent?.includes(m.onboarding_franchise_tour_1_title()) ?? false,
			'step 1 again'
		);
		action(m.onboarding_next())!.click();
		await until(() => action(m.onboarding_back()) !== null, 'step 2 again');
		action(m.onboarding_next())!.click();
		await until(() => action(m.onboarding_done()) !== null, 'the last step');
		expect(balloon()!.textContent).toContain(m.onboarding_franchise_tour_3_title());
		expect(action(m.onboarding_skip())).toBeNull();
		action(m.onboarding_done())!.click();
		await until(() => balloon() === null, 'closed');
		expect(isModalOpen()).toBe(false);
		await new Promise((r) => setTimeout(r, 50));
		expect(seen).toEqual([]);
	});

	it('Skip ends it from the first step', async () => {
		onboarding.showGuide(GUIDE);
		apps.push(mount(FranchisePage, { target }));
		apps.push(mount(OnboardHost, { target }));
		await shown();
		action(m.onboarding_skip())!.click();
		await until(() => balloon() === null, 'closed');
		expect(seen).toEqual([]);
	});
});

describe('a feature’s own Show me how', () => {
	it('runs the walkthrough on its page whenever it is pressed', async () => {
		apps.push(mount(FranchisePage, { target }));
		apps.push(mount(OnboardHost, { target }));
		apps.push(mount(ShowMeHow, { target, props: { howTo: GUIDE } }));
		const link = [...target.querySelectorAll('button')].find(
			(b) => b.textContent?.trim() === m.onboarding_show_how()
		)!;
		link.click();
		await shown();
		expect(balloon()!.textContent).toContain(m.onboarding_guide());
		action(m.onboarding_skip())!.click();
		await until(() => balloon() === null, 'closed');
		link.click();
		await shown();
		expect(seen).toEqual([]);
	});
});
