// A stand-in for Chromium's connection limit, for scenarios about
// requests that hold a connection.
//
// Chromium opens at most six connections to one origin over plain
// HTTP and queues every request past the sixth until one of those six
// answers, fails or is cancelled. jsdom has no such limit, so a request
// a page forgot to cancel costs nothing here unless a scenario puts
// one in front of `fetch`.

import { vi } from 'vitest';

/** Chromium's limit on connections to one origin over plain HTTP. */
export const CONNECTIONS = 6;

/** What went through the stand-in. */
export interface Connections {
	/** The paths of the requests the page cancelled, in order. */
	readonly cancelled: string[];
	/** Puts the real `fetch` back. */
	restore(): void;
}

interface Origin {
	open: number;
	readonly queue: Array<() => void>;
}

/** `fetch` behind six connections per origin, each freed as its request answers, fails or is cancelled. */
export function sixConnections(): Connections {
	const real = globalThis.fetch;
	const cancelled: string[] = [];
	const origins = new Map<string, Origin>();
	const originOf = (href: string): Origin => {
		const key = new URL(href).origin;
		let o = origins.get(key);
		if (!o) origins.set(key, (o = { open: 0, queue: [] }));
		return o;
	};
	vi.stubGlobal('fetch', (input: RequestInfo | URL, init?: RequestInit) => {
		const href = input instanceof Request ? input.url : String(input);
		const signal = init?.signal ?? (input instanceof Request ? input.signal : undefined);
		const origin = originOf(href);
		return new Promise<Response>((resolve, reject) => {
			let held = false;
			let done = false;
			const release = () => {
				if (!held) return;
				held = false;
				origin.open--;
				origin.queue.shift()?.();
			};
			const start = () => {
				if (done) return;
				held = true;
				origin.open++;
				real(input, init).then(
					(r) => {
						done = true;
						release();
						resolve(r);
					},
					(e) => {
						done = true;
						release();
						reject(e);
					}
				);
			};
			signal?.addEventListener(
				'abort',
				() => {
					if (done) return;
					done = true;
					cancelled.push(new URL(href).pathname);
					const queued = origin.queue.indexOf(start);
					if (queued >= 0) origin.queue.splice(queued, 1);
					release();
					reject(new DOMException('The operation was aborted.', 'AbortError'));
				},
				{ once: true }
			);
			if (origin.open < CONNECTIONS) start();
			else origin.queue.push(start);
		});
	});
	return {
		cancelled,
		restore: () => vi.unstubAllGlobals()
	};
}

/** The ids of the cancelled requests under `prefix` (`/api/kitsu/franchise/`), sorted. */
export function cancelledUnder(c: Connections, prefix: string): string[] {
	return c.cancelled
		.filter((p) => p.startsWith(prefix))
		.map((p) => decodeURIComponent(p.slice(prefix.length)))
		.sort();
}
