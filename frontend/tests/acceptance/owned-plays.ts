// The plays the routes run, seen through the module that judges them.
//
// Every play on the play and detail routes goes through `runOwnedPlay`,
// and the page acts on its outcome — navigates, shows a failure,
// records a watch — in the turn that outcome settles. A case that
// asserts the page did not act has to know it has had the outcome to
// act on; waiting a fixed time assumes the answer is in by then.
// Recording each outcome lets the case wait for the outcome itself:
// the page awaits the same promise and registered its wait first, so
// once it has settled the page's reaction has run.
//
// A played episode is recorded the same way: `recordPlayed` stamps
// Continue Watching and, where it may, mirrors the watch to the
// trackers; its promise settles once the stamp, and any tracker write
// it made, have been answered. A case that asserts no
// tracker was written waits for that promise rather than a fixed time.
//
// Install it from a case file with
//
//   vi.mock('$lib/play/owned-play', async (importOriginal) =>
//     (await import('./owned-plays')).trackOwnedPlays(await importOriginal())
//   );
//
// and clear, in `beforeEach`, the records the case reads.

import type * as OwnedPlayModule from '../../src/lib/play/owned-play';

/** Every play outcome the page asked for in the current case, in order. */
export const ownedPlays: Promise<unknown>[] = [];

/** Every played episode the page recorded in the current case, in
 *  order: each settles once its watch stamp, and any tracker write it
 *  made, have been answered. */
export const recordedPlays: { episode: number; written: Promise<void> }[] = [];

export function trackOwnedPlays(real: typeof OwnedPlayModule): typeof OwnedPlayModule {
	const runOwnedPlay = ((...args: Parameters<typeof real.runOwnedPlay>) => {
		const outcome = real.runOwnedPlay(...args);
		ownedPlays.push(outcome);
		return outcome;
	}) as typeof real.runOwnedPlay;
	const recordPlayed: typeof real.recordPlayed = (input, deps) => {
		const written = real.recordPlayed(input, deps);
		recordedPlays.push({ episode: input.episode, written });
		return written;
	};
	return { ...real, runOwnedPlay, recordPlayed };
}
