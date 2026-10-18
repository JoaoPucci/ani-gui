/**
 * What a clicked Continue row reads as once its numbering is settled
 * — the step of the resume click ($lib/history/start-resume) that
 * reads the row again, warming its own episode first when that is
 * what puts the row right.
 */

import { parseHistoryEpisode } from '$lib/play/episode-number';
import type { ResumePlayArgs, StartResumeDeps } from './start-resume';

/** The members of the resume click's dependencies this step uses. */
export type SettledRowDeps = Pick<StartResumeDeps, 'currentNumber' | 'warm'>;

/** What the clicked row reads as now, and the number it read as
 *  before its numbering was put right; what the card read when the row
 *  cannot be asked. */
async function rowNow(
	deps: SettledRowDeps,
	entryId: string,
	card: number | null
): Promise<{ last: number | null; former: number | null }> {
	try {
		const row = await deps.currentNumber?.(entryId);
		if (!row) return { last: card, former: null };
		return {
			last: parseHistoryEpisode(row.ep_no),
			former: parseHistoryEpisode(row.former_ep_no)
		};
	} catch {
		return { last: card, former: null };
	}
}

/** {@link rowNow}, with the row's own episode warmed first when no
 *  listing stands behind the cap (`unmeasured`) and the row reads as a
 *  fractional episode — the one kind a renumbering leaves under a
 *  number it no longer has. The warm resolves the listing, which puts
 *  the row right, and records no watch. A click the viewer moved on
 *  from by the time the row was read (`stillHere`) warms nothing. */
export async function settledRow(
	deps: SettledRowDeps,
	entryId: string,
	card: number | null,
	unmeasured: boolean,
	play: Omit<ResumePlayArgs, 'episode'>,
	stillHere: () => boolean
): Promise<{ last: number | null; former: number | null }> {
	const now = await rowNow(deps, entryId, card);
	if (!unmeasured || !deps.warm || now.last === null || Number.isInteger(now.last)) return now;
	if (!stillHere()) return now;
	try {
		await deps.warm({ ...play, episode: now.last });
	} catch {
		return now;
	}
	return rowNow(deps, entryId, now.last);
}
