/**
 * Whose turn the search page is: each search the runner starts takes a
 * turn, which ends the previous one's. A search checks its turn is
 * still current before anything of it reaches the page, and the
 * availability probes it asked are ended with it, since they would
 * hold the limiter's slots for results nobody will see.
 */

import type { ProbeRun } from '$lib/availability/probe-scope';

export interface Turn {
	/** Whether no search has taken a turn since this one. */
	current: () => boolean;
	/** The probes this turn's pages ask. */
	probes: ProbeRun;
}

export function searchTurns(beginProbes: () => ProbeRun): {
	take: () => Turn;
	end: () => void;
} {
	let generation = 0;
	let probes: ProbeRun | null = null;
	const end = () => {
		generation++;
		probes?.end();
		probes = null;
	};
	const take = (): Turn => {
		end();
		const mine = generation;
		probes = beginProbes();
		return { current: () => mine === generation, probes };
	};
	return { take, end };
}
