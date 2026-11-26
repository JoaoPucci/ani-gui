// Acceptance: the About page lists the feature tips offered in the
// running version, seen or not, each with its title, the version that
// brought it and what it says, so a tip closed in passing can be read
// again. With no tips offered, or when they cannot be read, the section
// is not there.

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

import AboutPage from '../../src/routes/about/+page.svelte';
import { __resetApiBaseForTests, type OnboardingView } from '../../src/lib/api';
import { __resetOnboardingForTests } from '../../src/lib/onboarding/tips.svelte';
import { m } from '../../src/lib/paraglide/messages';
import { errorAnswer } from '../answers';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

const VIEW = (pending: boolean): OnboardingView => ({
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

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	__resetOnboardingForTests();
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
});

const section = () => target.querySelector<HTMLElement>('[data-testid="about-in-this-version"]');

async function settle() {
	for (let i = 0; i < 20; i++) await new Promise((r) => setTimeout(r, 10));
}

describe('the About page’s new features', () => {
	it('lists each tip offered, seen or not, with its version and words', async () => {
		for (const pending of [true, false]) {
			__resetOnboardingForTests();
			server.use(
				http.get(`${API_BASE}/api/onboarding`, () =>
					HttpResponse.json<OnboardingView>(VIEW(pending))
				)
			);
			app = mount(AboutPage, { target });
			await settle();
			const text = section()?.textContent ?? '';
			expect(text, `pending ${pending}`).toContain(m.about_section_in_this_version_title());
			expect(text).toContain(m.onboarding_watch_order_1_title());
			expect(text).toContain(m.onboarding_watch_order_1_body());
			expect(text).toContain('0.19.0');
			unmount(app);
			app = null;
		}
	});

	it('leaves the section out with nothing offered, or when the tips cannot be read', async () => {
		server.use(
			http.get(`${API_BASE}/api/onboarding`, () =>
				HttpResponse.json<OnboardingView>({ version: '0.21.0', tips: [] })
			)
		);
		app = mount(AboutPage, { target });
		await settle();
		expect(section()).toBeNull();
		unmount(app);
		__resetOnboardingForTests();
		server.use(http.get(`${API_BASE}/api/onboarding`, () => errorAnswer(503)));
		app = mount(AboutPage, { target });
		await settle();
		expect(section()).toBeNull();
	});
});
