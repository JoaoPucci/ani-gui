// What a page asks of the network, seen from the page's side of it.
//
// A handler sees a request only once MSW has carried it there, which
// takes an unknown number of turns, so "no handler saw it" after a
// fixed wait says nothing on a slow run. These records are taken
// where the page acts instead: a request when the page calls
// `fetch`, an answer when the page reads its body, and a request
// that never answers when its `fetch` or the read of its body
// rejects (an abort among them).
// A case can then wait for the answer its claim is about to be read,
// and check what the page asked for after it, without guessing how
// long the network takes.
//
// Where a claim follows every answer rather than one, `settled` waits
// until nothing the page asked is still out or being read across a
// whole turn of the event loop. A request still out keeps the page
// unsettled however long it takes: the difference from a fixed sleep,
// which ends on time whether or not the answer has landed.
//
// The tier's teardown (`setup.ts`) already fails a case whose request
// is still out when its page unmounts. What it cannot see is an answer
// that lands after a sleep ended but before the case did — one the
// case itself goes on to wait for — and that is what these close.

import { onTestFinished, vi } from 'vitest';
import { tick } from 'svelte';

/** One request as the page sent it: its method, path, query and JSON
 *  body. */
export interface PageFetch {
	method: string;
	path: string;
	query: URLSearchParams;
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
	const url = new URL(href, location.href);
	return { method: method.toUpperCase(), path: url.pathname, query: url.searchParams, body };
}

/**
 * Record, for the rest of the case, every request the page sends
 * (`sent`, as `fetch` is called), every answer whose body it has read
 * (`read`, once the read resolves) and every request whose `fetch` or
 * body read rejected (`failed`, an aborted one among them).
 */
export function watchPageFetches(): {
	sent: PageFetch[];
	read: PageFetch[];
	failed: PageFetch[];
	/** Resolves once nothing the page asked is out or being read across
	 *  a whole turn: every answer is in and read, and what it set off
	 *  has asked whatever it asks. */
	settled(what: string, timeoutMs?: number): Promise<void>;
} {
	const sent: PageFetch[] = [];
	const read: PageFetch[] = [];
	const failed: PageFetch[] = [];
	/** Requests not yet answered, plus bodies being read. */
	let out = 0;
	const realFetch = globalThis.fetch;
	vi.stubGlobal('fetch', async (input: RequestInfo | URL, init?: RequestInit) => {
		const asked = described(input, init);
		sent.push(asked);
		out += 1;
		let resp: Response;
		try {
			resp = await realFetch(input, init);
		} catch (e) {
			failed.push(asked);
			throw e;
		} finally {
			out -= 1;
		}
		for (const how of ['text', 'json', 'arrayBuffer', 'blob'] as const) {
			const original = resp[how].bind(resp) as () => Promise<unknown>;
			Object.defineProperty(resp, how, {
				configurable: true,
				value: async () => {
					out += 1;
					try {
						const value = await original();
						read.push(asked);
						return value;
					} catch (e) {
						// A body that never arrives, an abort mid-read among
						// them, is an answer that failed.
						failed.push(asked);
						throw e;
					} finally {
						out -= 1;
					}
				}
			});
		}
		return resp;
	});
	onTestFinished(() => {
		vi.unstubAllGlobals();
	});
	async function settled(what: string, timeoutMs = 8000) {
		const deadline = Date.now() + timeoutMs;
		for (;;) {
			if (out === 0) {
				await new Promise((r) => setTimeout(r, 0));
				if (out === 0) return;
			} else {
				await new Promise((r) => setTimeout(r, 10));
			}
			if (Date.now() > deadline) {
				throw new Error(`timed out waiting for ${what}: the page still has a request out`);
			}
		}
	}
	return { sent, read, failed, settled };
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
