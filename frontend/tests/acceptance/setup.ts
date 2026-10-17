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
	http.post(`${API_BASE}/api/play`, async ({ request, requestId }) => {
		const body = (await request
			.clone()
			.json()
			.catch(() => null)) as { prefetch?: boolean } | null;
		if (body?.prefetch !== true) undescribedRequest(request, requestId);
		return absentBackend();
	})
];

/**
 * Last of all, behind every handler a scenario installs (`server.use`
 * puts those first): anything still unanswered here — no handler
 * matched it, or every one that did returned nothing, which in MSW
 * falls through to the next — is undescribed, and fails as a network
 * error rather than reaching the network.
 */
const undescribedAnswer = http.all('*', ({ request, requestId }) => {
	undescribedRequest(request, requestId);
	return HttpResponse.error();
});

/** Started with only the defaults above — each scenario installs its own. */
// Every mounted play page asks the proxy for its session's sidecar
// tracks; by default there are none. A test that wants tracks
// overrides this per session.
export const server = setupServer(
	http.get(`${API_BASE}/s/:session/subtitles`, () => HttpResponse.json([])),
	...incidental,
	undescribedAnswer
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
// timer, promise and fetch it starts inherits, however late they run,
// and every DOM event listener it adds is bound to: a request sent
// from those carries its scenario across the boundary into the next
// one. A scenario fails on:
//
// - a request of its own that no handler answered: one that reached
//   the final catch-all, one a handler let through with
//   `passthrough()`, or one whose handler threw;
// - a request of its own sent, or still unanswered, after its
//   teardown unmounted its page;
// - such a late request from an earlier scenario, or one sent outside
//   any scenario, seen while it ran — named as such.
//
// What it cannot see:
//
// - a late request after the last scenario of a file, later than the
//   file's grace below, once the server has closed;
// - work whose async context was not created in a scenario's body or
//   its `beforeEach`/`afterEach`: a timer started at import, or a
//   request sent from `beforeAll`/`afterAll`, counts as sent outside
//   any scenario, even when a handler answers it;
// - a callback a page leaves registered somewhere other than a DOM
//   event target — a store subscription, a callback queue — that a
//   later scenario triggers: it runs in the context of whatever
//   triggers it, and its request is taken for that scenario's. DOM
//   event listeners are bound to the scenario that added them (see
//   `bindListenersToTheirScenario`), so a leaked one is caught;
// - a WebSocket, which raises no request events at all (the app opens
//   none).

interface Scenario {
	name: string;
	/** The scenario's teardown has unmounted its page. */
	ended: boolean;
}

const owner = new AsyncLocalStorage<Scenario>();
/** The scenario each request in flight was sent by, by request id. */
const senders = new Map<string, Scenario | undefined>();
/** Requests a scenario sent that nothing answered, by scenario. */
const undescribed = new Map<Scenario, string[]>();
/** Requests sent by a scenario that had ended, or outside any. */
const strays: string[] = [];

/** How long the file's teardown waits for a late request before MSW
 *  stops intercepting. A request later than this is not caught; it
 *  would reach the closed port. */
const LATE_REQUEST_GRACE_MS = 100;

const label = (request: Request) => `${request.method} ${request.url}`;

function stray(request: Request, sender: Scenario | undefined, when: string) {
	strays.push(
		sender === undefined
			? `${label(request)} — sent outside any scenario`
			: `${label(request)} — ${when} "${sender.name}" had unmounted its page`
	);
}

function undescribedRequest(request: Request, requestId: string) {
	const sender = senders.has(requestId) ? senders.get(requestId) : owner.getStore();
	if (sender === undefined || sender.ended) {
		stray(request, sender, 'unanswered after');
		return;
	}
	const seen = undescribed.get(sender) ?? [];
	seen.push(label(request));
	undescribed.set(sender, seen);
}

server.events.on('request:start', ({ request, requestId }) => {
	const sender = owner.getStore();
	senders.set(requestId, sender);
	if (sender === undefined || sender.ended) stray(request, sender, 'sent after');
});
// `passthrough()` sends the request on to the network.
server.events.on('response:bypass', ({ request, requestId }) =>
	undescribedRequest(request, requestId)
);
// A handler that throws answers with a 500, which reads as the
// backend's own; it is the scenario's error, not the backend's.
server.events.on('unhandledException', ({ request, requestId }) =>
	undescribedRequest(request, requestId)
);
server.events.on('request:end', ({ requestId }) => {
	senders.delete(requestId);
});

/**
 * Binds every DOM event listener to the scenario that added it, so a
 * listener a page leaves behind, fired by a later scenario, still
 * sends as the page's own. A listener is bound when it is added and
 * remembered per target, type and phase, so the matching remove finds
 * it; once removed, the same function added again — as Svelte re-adds
 * its delegated handler on each mount — is bound afresh, to the
 * scenario adding it then. One re-added without being removed (after a
 * `once` fired, say) keeps the scenario that first added it. Only function listeners; a listener object's
 * `handleEvent` keeps the caller's context.
 *
 * Patched where the listeners land: the EventTarget every happy-dom
 * node inherits from, and the window, whose global carries its own
 * `addEventListener` under vitest.
 */
function bindListenersToTheirScenario() {
	const bound = new WeakMap<object, Map<string, Map<unknown, EventListener>>>();
	const slot = (target: object, type: string, options?: boolean | EventListenerOptions) => {
		const capture = typeof options === 'boolean' ? options : Boolean(options?.capture);
		let byKey = bound.get(target);
		if (!byKey) {
			byKey = new Map();
			bound.set(target, byKey);
		}
		const key = `${type}|${capture}`;
		let byListener = byKey.get(key);
		if (!byListener) {
			byListener = new Map();
			byKey.set(key, byListener);
		}
		return byListener;
	};
	const patch = (target: {
		addEventListener: EventTarget['addEventListener'];
		removeEventListener: EventTarget['removeEventListener'];
	}) => {
		const add = target.addEventListener;
		const remove = target.removeEventListener;
		target.addEventListener = function (this: EventTarget, type, listener, options) {
			if (typeof listener !== 'function' || owner.getStore() === undefined) {
				return add.call(this, type, listener, options);
			}
			const listeners = slot(this, type, options);
			// Added before and not removed since: hand the DOM the same
			// bound listener, which it ignores if still registered and
			// takes back if a `once` let it go.
			let b = listeners.get(listener);
			if (!b) {
				b = AsyncLocalStorage.bind(listener) as EventListener;
				listeners.set(listener, b);
			}
			return add.call(this, type, b, options);
		};
		target.removeEventListener = function (this: EventTarget, type, listener, options) {
			const listeners = slot(this, type, options);
			const b = listeners.get(listener);
			if (b) {
				listeners.delete(listener);
				return remove.call(this, type, b, options);
			}
			return remove.call(this, type, listener, options);
		};
	};
	patch(Object.getPrototypeOf(window.Node.prototype));
	patch(window);
}
bindListenersToTheirScenario();

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
