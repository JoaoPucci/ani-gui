// Shared setup for the route-level acceptance tier.
//
// Two fakes and nothing else, so a scenario exercises the real
// modules between them:
//
//   - MSW owns the HTTP boundary. The renderer reaches the Rust
//     sidecar over `fetch()` against a localhost origin, so an
//     interceptor is enough — there is no IPC to stub for data.
//   - `window.aniGui` is the preload bridge, which MSW cannot reach.
//     Only the members a route actually touches are faked; anything
//     else should fail loudly rather than silently return undefined.

import { afterAll, afterEach, beforeAll, beforeEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { setupServer } from 'msw/node';

import { installElementAnimateStub } from './stubs/element-animate';

// Before any component mounts. See the stub for why happy-dom needs
// it and why only post-mount DOM changes trip over its absence.
installElementAnimateStub();

/**
 * The origin every handler is written against. Real runs discover a
 * random port from the preload bridge; a test pins it so the handlers
 * can use absolute URLs.
 */
export const API_BASE = 'http://127.0.0.1:31337';

/**
 * Requests pages send on the side of what most scenarios describe,
 * answered as a backend that is not there: a network failure, which
 * is what every one of them got before a request no handler answered
 * failed its scenario. Each page already treats that failure as
 * "nothing to show", so naming them here changes no scenario; it only
 * keeps them from counting as undescribed. A scenario that cares
 * about one installs its own handler, which takes precedence.
 *
 * - the download folder the detail and play pages show beside the
 *   download action;
 * - a show's watch history, which the detail page reads for resume;
 * - the title search a page runs to name a show's provider entry;
 * - the availability reads and warms the home and list rows send;
 * - a signed-in tracker list a list row reads;
 * - the background warm of an episode the user has not asked for:
 *   with no EventSource installed it is a plain `/api/play` POST
 *   marked `prefetch`. A play the user asked for carries no
 *   `prefetch`, and still has to be described.
 */
const absentBackend = () => HttpResponse.error();
const incidental = [
	http.get(`${API_BASE}/api/download/default-dir`, absentBackend),
	http.get(`${API_BASE}/api/history/by-kitsu/:id`, absentBackend),
	http.post(`${API_BASE}/api/kitsu/search`, absentBackend),
	http.post(`${API_BASE}/api/availability/batch`, absentBackend),
	http.post(`${API_BASE}/api/availability/warm`, absentBackend),
	http.post(`${API_BASE}/api/account/list/:provider`, absentBackend),
	http.post(`${API_BASE}/api/play`, async ({ request }) => {
		const body = (await request
			.clone()
			.json()
			.catch(() => null)) as { prefetch?: boolean } | null;
		return body?.prefetch === true ? absentBackend() : undefined;
	})
];

/** Started with only the defaults above — each scenario installs its own. */
// Every mounted play page asks the proxy for its session's sidecar
// tracks; by default there are none. A test that wants tracks
// overrides this per session.
export const server = setupServer(
	http.get(`${API_BASE}/s/:session/subtitles`, () => HttpResponse.json([])),
	...incidental
);

// What the HTTP boundary saw that no scenario accounts for. MSW's
// own `error` strategy only fails the request, and the page under
// test catches that, so on its own a request no handler answered
// goes unnoticed while the scenario passes; and a request a page
// sends after its scenario ended — a queued warm, a timer — reaches
// the real port once the server has closed. Both fail a scenario
// here instead.

/** Requests no handler answered while a scenario ran. */
const undescribed: string[] = [];
/** Every request, answered or not, sent while no scenario ran: after
 *  one scenario's teardown and before the next one starts, or after
 *  the file's last. Whatever sends it outlived the page that
 *  mounted it. */
const afterItsScenario: string[] = [];
let scenarioRunning = false;

/** How long the file's teardown waits for a request the last
 *  scenario's page sends late before MSW stops intercepting. A request
 *  later than this is not caught; it would reach the closed port. */
const LATE_REQUEST_GRACE_MS = 100;

function failOn(seen: string[], what: string) {
	if (seen.length === 0) return;
	throw new Error(`${what}:\n${seen.splice(0).join('\n')}`);
}

server.events.on('request:start', ({ request }) => {
	if (!scenarioRunning) afterItsScenario.push(`${request.method} ${request.url}`);
});
server.events.on('request:unhandled', ({ request }) => {
	if (scenarioRunning) undescribed.push(`${request.method} ${request.url}`);
});

beforeAll(() => {
	// `error` rather than `warn`: an unhandled request means the
	// scenario is reaching a surface it did not describe, and a
	// silently-empty response would show up later as an unexplained
	// assertion failure somewhere else entirely.
	server.listen({ onUnhandledRequest: 'error' });
});

// Registered before every scenario's own hooks, so this `beforeEach`
// runs before them and this `afterEach` after them: the page has
// been unmounted by the time the scenario stops counting.
beforeEach(() => {
	failOn(afterItsScenario, 'requests sent after the previous scenario ended');
	scenarioRunning = true;
});

afterEach(async () => {
	// The scenario's own teardown has run and its page is gone, so from
	// here a request is one that outlived it. One turn for what the
	// unmount set off.
	scenarioRunning = false;
	await new Promise((r) => setTimeout(r, 0));
	server.resetHandlers();
	failOn(undescribed, 'requests no handler answered during this scenario');
	failOn(afterItsScenario, "requests sent after this scenario's page was unmounted");
});

afterAll(async () => {
	await new Promise((r) => setTimeout(r, LATE_REQUEST_GRACE_MS));
	try {
		failOn(afterItsScenario, "requests sent after this file's last scenario ended");
	} finally {
		server.close();
	}
});

beforeAll(() => {
	// `api.ts` prefers `window.aniGui.apiBase` and caches it on first
	// read, so this has to exist before any module reads it.
	Object.defineProperty(window, 'aniGui', {
		configurable: true,
		writable: true,
		value: {
			apiBase: API_BASE,
			platform: 'linux'
		}
	});
});
