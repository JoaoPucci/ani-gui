// What the guard fixtures share: a handler that answers, one that
// answers late, and one that never does.

import { http, HttpResponse } from 'msw';

import { API_BASE, server } from '../../acceptance/setup';

export { API_BASE };

export const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** `/ok` answers at once, `/slow` after `ms`, `/never` not at all. */
export function useHandlers(ms = 40) {
	server.use(
		http.get(`${API_BASE}/ok`, () => HttpResponse.json({})),
		http.get(`${API_BASE}/slow`, async () => {
			await wait(ms);
			return HttpResponse.json({});
		}),
		http.get(`${API_BASE}/never`, () => new Promise<never>(() => {}))
	);
}

/** A request whose outcome the page ignores, as a leftover would. */
export function send(path: string, init?: RequestInit) {
	fetch(`${API_BASE}${path}`, init).catch(() => {});
}
