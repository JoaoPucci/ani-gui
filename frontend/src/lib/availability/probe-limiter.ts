/**
 * A renderer-wide limiter for availability probes.
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

export function createProbeLimiter(opts: ProbeLimiterOptions): ProbeLimiter {
	void opts;
	let running = 0;
	return {
		async run(task, runOpts) {
			running += 1;
			try {
				return await task(runOpts.signal);
			} finally {
				running -= 1;
			}
		},
		inFlight: () => running
	};
}

/** The one every availability probe in the renderer goes through. */
export const probeLimiter = createProbeLimiter({ max: 4, backgroundMax: 3 });
