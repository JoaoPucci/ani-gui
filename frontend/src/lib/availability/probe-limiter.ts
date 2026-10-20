/**
 * A renderer-wide limiter for availability probes.
 *
 * A probe holds its connection until the provider walk answers or its
 * sixty-second deadline passes, and several surfaces fire them at once
 * — the search page's pool, the home page's Continue Watching rows,
 * the detail and play pages' own lookups — with nothing bounding the
 * total. The probe lane gives them a connection budget of their own,
 * so they no longer queue the player or the API; this bounds what they
 * take of that budget, and orders it:
 *
 *   - at most `max` probes run at once, renderer-wide;
 *   - background probes (cache fills, `background: true`) take at most
 *     `backgroundMax` of those, so a probe the user is waiting on — a
 *     click's recheck, a page's own lookup — always finds a slot;
 *   - a waiting interactive probe starts before a waiting background
 *     one;
 *   - a probe whose signal aborts while it waits leaves the queue and
 *     never runs, and a running one is handed its signal, so leaving a
 *     page frees the slots its probes held.
 *
 * The backend paces provider traffic again through its own admission
 * gate, so a cap here moves the queue into the renderer rather than
 * slowing the walks, and a queued probe is one abort can still drop.
 */

export interface ProbeLimiterOptions {
	max: number;
	backgroundMax: number;
}

export interface ProbeRunOptions {
	background: boolean;
	signal?: AbortSignal;
}

export interface ProbeLimiter {
	run<T>(task: (signal: AbortSignal | undefined) => Promise<T>, opts: ProbeRunOptions): Promise<T>;
	inFlight(): number;
}

interface Waiting {
	background: boolean;
	start: () => void;
}

// i18n-ignore: a DOMException's message and name, never shown to the user
const abortError = () => new DOMException('The probe was aborted.', 'AbortError');

export function createProbeLimiter(opts: ProbeLimiterOptions): ProbeLimiter {
	let running = 0;
	let runningBackground = 0;
	const interactive: Waiting[] = [];
	const background: Waiting[] = [];

	const pump = () => {
		while (running < opts.max) {
			if (interactive.length > 0) {
				interactive.shift()!.start();
			} else if (background.length > 0 && runningBackground < opts.backgroundMax) {
				background.shift()!.start();
			} else {
				return;
			}
		}
	};

	const slot = (runOpts: ProbeRunOptions): Promise<void> =>
		new Promise<void>((resolve, reject) => {
			const { signal } = runOpts;
			if (signal?.aborted) return reject(abortError());
			const queue = runOpts.background ? background : interactive;
			const onAbort = () => {
				const at = queue.indexOf(entry);
				if (at >= 0) queue.splice(at, 1);
				reject(abortError());
			};
			const entry: Waiting = {
				background: runOpts.background,
				start: () => {
					signal?.removeEventListener('abort', onAbort);
					running += 1;
					if (runOpts.background) runningBackground += 1;
					resolve();
				}
			};
			signal?.addEventListener('abort', onAbort, { once: true });
			queue.push(entry);
			pump();
		});

	return {
		async run(task, runOpts) {
			await slot(runOpts);
			try {
				return await task(runOpts.signal);
			} finally {
				running -= 1;
				if (runOpts.background) runningBackground -= 1;
				pump();
			}
		},
		inFlight: () => running
	};
}

/** The one every availability probe in the renderer goes through.
 *  Four matches the pools the surfaces ran on their own, and leaves
 *  two of the probe lane's six connections free. */
export const probeLimiter = createProbeLimiter({ max: 4, backgroundMax: 3 });
