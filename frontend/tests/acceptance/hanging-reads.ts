// Reads a slow backend holds open, for scenarios about a page left
// while they are still out.
//
// A handler made with `hang` answers nothing until the scenario ends:
// the request holds its connection as one the backend is still working
// on does (Kitsu slow, a tracker slow, the database busy). Each one the
// page sent is named in `held`, by method and path, so a scenario can
// check that every read it held was cancelled when the page went
// (`./connections`). `release` answers them all with a failure, for
// the scenario's teardown.

import { HttpResponse } from 'msw';

export interface HangingReads {
	/** A handler body that holds its request until `release`. */
	hang: (info: { request: Request }) => Promise<Response>;
	/** Every held request, as `METHOD /path`, in the order it arrived. */
	readonly held: string[];
	/** Answer every held request (a 503), so nothing is left pending. */
	release(): void;
}

export function hangingReads(): HangingReads {
	const held: string[] = [];
	const ends: Array<() => void> = [];
	return {
		held,
		hang: ({ request }) =>
			new Promise<Response>((resolve) => {
				held.push(`${request.method} ${new URL(request.url).pathname}`);
				ends.push(() => resolve(new HttpResponse(null, { status: 503 })));
			}),
		release: () => {
			for (const end of ends.splice(0)) end();
		}
	};
}

/** `held`'s paths, without the method, sorted: what `./connections`
 *  records a cancelled request by. */
export function heldPaths(reads: HangingReads): string[] {
	return reads.held.map((h) => h.slice(h.indexOf(' ') + 1)).sort();
}
