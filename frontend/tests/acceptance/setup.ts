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

import { AsyncLocalStorage } from 'node:async_hooks';
import { afterAll, afterEach, aroundEach, beforeAll } from 'vitest';
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
 *   `prefetch`: this handler answers it too, so it cannot slip past
 *   to the real port, but records it as undescribed, which fails the
 *   scenario that sent it.
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
		if (body?.prefetch !== true) undescribedRequest(request);
		return absentBackend();
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

// What the HTTP boundary saw that no scenario accounts for, and the
// scenario it belongs to.
//
// MSW's own `error` strategy only fails a request no handler answers,
// and the page under test catches that, so on its own such a request
// goes unnoticed while the scenario passes. A request a page sends
// after its scenario ended — a queued warm, a timer — is worse: it is
// answered by whatever handlers the next scenario installed, or
// reaches the real port once the server has closed.
//
// So every request is attributed to the scenario that caused it. Each
// scenario runs inside an AsyncLocalStorage context, which every
// timer, promise and fetch it starts inherits, however late they
// run; a request carries its sender's scenario across the boundary
// into the next one. A scenario fails on:
//
// - a request of its own that no handler answered, or that a handler
//   let through to the network (matched, but returned no response);
// - a request of its own sent after its teardown unmounted its page;
// - such a late request from an earlier scenario, or one sent outside
//   any scenario, seen while it ran — named as such.
//
// What it cannot see: a late request sent after the last scenario of
// a file ended, later than the file's grace below, when the server
// has closed; and work whose async context was not created inside a
// scenario (module-level timers started at import, which would show
// up as "outside any scenario" only if they fire while one runs).

interface Scenario {
	name: string;
	/** The scenario's teardown has unmounted its page. */
	ended: boolean;
}

const owner = new AsyncLocalStorage<Scenario>();
const matched = new Set<string>();
/** Requests a scenario sent that nothing answered, by scenario. */
const undescribed = new Map<Scenario, string[]>();
/** Requests sent by a scenario that had ended, or outside any. */
const strays: string[] = [];

/** How long the file's teardown waits for a late request before MSW
 *  stops intercepting. A request later than this is not caught; it
 *  would reach the closed port. */
const LATE_REQUEST_GRACE_MS = 100;

const label = (request: Request) => `${request.method} ${request.url}`;

function undescribedRequest(request: Request) {
	const sender = owner.getStore();
	if (sender === undefined || sender.ended) return; // a stray; counted as one
	const seen = undescribed.get(sender) ?? [];
	seen.push(label(request));
	undescribed.set(sender, seen);
}

server.events.on('request:start', ({ request }) => {
	const sender = owner.getStore();
	if (sender === undefined) strays.push(`${label(request)} — sent outside any scenario`);
	else if (sender.ended)
		strays.push(`${label(request)} — sent by "${sender.name}" after its page was unmounted`);
});
server.events.on('request:match', ({ requestId }) => {
	matched.add(requestId);
});
server.events.on('request:unhandled', ({ request }) => undescribedRequest(request));
server.events.on('request:end', ({ request, requestId }) => {
	// Ended without a response from any handler: passed through.
	if (!matched.delete(requestId)) undescribedRequest(request);
});

function failOn(seen: string[] | undefined, what: string) {
	if (seen === undefined || seen.length === 0) return;
	throw new Error(`${what}:\n${seen.splice(0).join('\n')}`);
}

beforeAll(() => {
	// `error` rather than `warn`: an unhandled request means the
	// scenario is reaching a surface it did not describe, and a
	// silently-empty response would show up later as an unexplained
	// assertion failure somewhere else entirely.
	server.listen({ onUnhandledRequest: 'error' });
});

aroundEach(async (runScenario, context) => {
	await owner.run({ name: context.task.name, ended: false }, runScenario);
});

// Registered before every scenario's own hooks, so this `afterEach`
// runs after them: the page has been unmounted by the time the
// scenario stops counting.
afterEach(async () => {
	const scenario = owner.getStore();
	if (scenario) scenario.ended = true;
	// One turn for what the unmount set off.
	await new Promise((r) => setTimeout(r, 0));
	server.resetHandlers();
	try {
		failOn(
			scenario && undescribed.get(scenario),
			'requests no handler answered during this scenario'
		);
	} finally {
		if (scenario) undescribed.delete(scenario);
	}
	failOn(strays, 'requests sent by no live scenario');
});

afterAll(async () => {
	await new Promise((r) => setTimeout(r, LATE_REQUEST_GRACE_MS));
	try {
		failOn(strays, "requests sent by no live scenario, after this file's last one ended");
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
