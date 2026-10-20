// What a page asks of the network, seen from the page's side of it.
//
// A handler sees a request only once MSW has carried it there, which
// takes an unknown number of turns, so "no handler saw it" after a
// fixed wait says nothing on a slow run. These records are taken
// where the page acts instead: a request when the page calls
// `fetch`, an answer when the page reads its body, and a request
// that never answers when its `fetch` rejects (an abort among them).
// A case can then wait for the answer its claim is about to be read,
// and check what the page asked for after it, without guessing how
// long the network takes.

import { onTestFinished, vi } from 'vitest';
import { tick } from 'svelte';

/** One request as the page sent it: its method, path and JSON body. */
export interface PageFetch {
	method: string;
	path: string;
	body: Record<string, unknown>;
}

function described(input: RequestInfo | URL, init?: RequestInit): PageFetch {
	const href = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url;
	const method = init?.method ?? (input instanceof Request ? input.method : 'GET');
	let body: Record<string, unknown> = {};
	if (typeof init?.body === 'string') {
		try {
			body = JSON.parse(init.body) as Record<string, unknown>;
		} catch {
			body = {};
		}
	}
	return { method: method.toUpperCase(), path: new URL(href).pathname, body };
}

/**
 * Record, for the rest of the case, every request the page sends
 * (`sent`, as `fetch` is called), every answer whose body it has read
 * (`read`, once the read resolves) and every request whose `fetch`
 * rejected (`failed`, an aborted one among them).
 */
export function watchPageFetches(): {
	sent: PageFetch[];
	read: PageFetch[];
	failed: PageFetch[];
} {
	const sent: PageFetch[] = [];
	const read: PageFetch[] = [];
	const failed: PageFetch[] = [];
	const realFetch = globalThis.fetch;
	vi.stubGlobal('fetch', async (input: RequestInfo | URL, init?: RequestInit) => {
		const asked = described(input, init);
		sent.push(asked);
		let resp: Response;
		try {
			resp = await realFetch(input, init);
		} catch (e) {
			failed.push(asked);
			throw e;
		}
		for (const how of ['text', 'json'] as const) {
			const original = resp[how].bind(resp) as () => Promise<unknown>;
			Object.defineProperty(resp, how, {
				configurable: true,
				value: async () => {
					const value = await original();
					read.push(asked);
					return value;
				}
			});
		}
		return resp;
	});
	onTestFinished(() => {
		vi.unstubAllGlobals();
	});
	return { sent, read, failed };
}

/**
 * Let what a read answer (or a failed request) sets off run to its
 * next request. Where the page works in memory from there to its next
 * `fetch`, one turn of the event loop between two Svelte flushes
 * carries it there, and the request is recorded as it is asked. A
 * page that waits on a timer of its own first is not covered.
 */
export async function afterRead(): Promise<void> {
	await tick();
	await new Promise((r) => setTimeout(r, 0));
	await tick();
}
