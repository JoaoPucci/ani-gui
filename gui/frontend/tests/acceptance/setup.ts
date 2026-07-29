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

import { afterAll, afterEach, beforeAll } from 'vitest';
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

/** Started with no handlers — each scenario installs its own. */
export const server = setupServer();

beforeAll(() => {
	// `error` rather than `warn`: an unhandled request means the
	// scenario is reaching a surface it did not describe, and a
	// silently-empty response would show up later as an unexplained
	// assertion failure somewhere else entirely.
	server.listen({ onUnhandledRequest: 'error' });
});

afterEach(() => {
	server.resetHandlers();
});

afterAll(() => {
	server.close();
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
