// Acceptance: which episode a Continue card offers, and which cap it
// trusts to decide that.
//
// `pickNextEpisode` advances to `lastWatched + 1` unless that would
// pass the cap, in which case it offers the last episode again. The
// cap it uses is the PROBED playable count in preference to Kitsu's
// announced `episode_count`, because the announced number routinely
// runs ahead of what the provider actually carries — advancing on it
// forwards a phantom episode into an episode-not-released error where
// the probed cap would have replayed the finale.
//
// The number on the card therefore depends on the history row, the
// Kitsu match and the availability probe agreeing, which is why this
// belongs at the route.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { homeHandlers, kitsuRef } from './home-handlers';

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
import { m } from '../../src/lib/paraglide/messages';

const SHOW = 'Cowboy Bebop';
const ANNOUNCED = 26;

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

const carrying = (el: Element | null) => el?.textContent?.includes(SHOW) ?? false;
/** The resolved row. Only this branch renders a button. */
const resumeButton = () => Array.from(target.querySelectorAll('button')).find(carrying) ?? null;

/**
 * Mount the home route for one history row and wait until it has
 * resolved AND its availability probe has been answered — both, since
 * the probed cap is what the offered episode is supposed to hinge on
 * and the card renders once before that lands.
 */
async function mountRow(opts: { watched: number; probedCap: number }) {
	let probeAnswered = false;
	server.use(
		...homeHandlers({ history: [{ ep_no: String(opts.watched), id: 'allanime-1', title: SHOW }] }, [
			http.post(`${API_BASE}/api/kitsu/search`, () =>
				HttpResponse.json([kitsuRef('1', SHOW, ANNOUNCED)])
			),
			http.post(`${API_BASE}/api/availability`, () => {
				probeAnswered = true;
				return HttpResponse.json({
					available: true,
					episode_count: opts.probedCap,
					approximate: false
				});
			})
		])
	);

	app = mount(HomePage, { target });
	await until(() => resumeButton() !== null, 'the row to resolve into a resume button');
	await until(() => probeAnswered, 'the availability probe to be answered');
}

/** The episode the card is offering, read from its rendered label. */
const offers = (episode: number) =>
	resumeButton()?.textContent?.includes(m.home_resume_episode_label({ episode })) ?? false;

describe('Continue Watching offered episode', () => {
	it('advances to the next episode mid-season', async () => {
		await mountRow({ watched: 3, probedCap: ANNOUNCED });
		await until(() => offers(4), 'the card to offer episode 4');
		expect(offers(3)).toBe(false);
	});

	it('replays the last episode when the row sits at the cap', async () => {
		await mountRow({ watched: ANNOUNCED, probedCap: ANNOUNCED });
		await until(() => offers(ANNOUNCED), `the card to offer episode ${ANNOUNCED} again`);
		expect(offers(ANNOUNCED + 1)).toBe(false);
	});

	it('replays at the PROBED cap, not the higher announced one', async () => {
		// Kitsu announces 26; the provider actually carries 12. Trusting
		// the announced number here would offer episode 13, which does
		// not exist — the error this rule exists to avoid.
		await mountRow({ watched: 12, probedCap: 12 });
		await until(() => offers(12), 'the card to replay the last playable episode');
		expect(offers(13)).toBe(false);
	});
});
