/**
 * Validation for the Download confirm modal's Range inputs. Returns
 * null when the range is valid and a message for the inline error row
 * when it is not; the modal also disables Confirm while it is non-null.
 * Only Range mode is checked — This and All build their episode arg
 * from constants.
 *
 * `rangeMax` is the highest episode the range may reach: the show's
 * known episode count, or the modal's fallback cap when the count is
 * unknown (`maxEpisode` null).
 */
export function rangeError(input: {
	mode: 'this' | 'all' | 'range';
	startEp: number;
	endEp: number;
	rangeMax: number;
	maxEpisode: number | null;
}): string | null {
	const { mode, startEp, endEp, rangeMax, maxEpisode } = input;
	if (mode !== 'range') return null;
	const s = Math.floor(startEp);
	const e = Math.floor(endEp);
	if (!Number.isFinite(startEp) || !Number.isFinite(endEp)) {
		return 'Enter a number for both From and To.';
	}
	if (s < 1 || e < 1) return 'Episode numbers must be at least 1.';
	if (s > rangeMax) {
		return maxEpisode
			? `Only ${maxEpisode} episode${maxEpisode === 1 ? '' : 's'} available — From can't exceed ${maxEpisode}.`
			: `You can't download more than ${rangeMax} episodes for this show.`;
	}
	if (e > rangeMax) {
		return maxEpisode
			? `Only ${maxEpisode} episode${maxEpisode === 1 ? '' : 's'} available — To can't exceed ${maxEpisode}.`
			: `You can't download more than ${rangeMax} episodes for this show.`;
	}
	if (e < s) return 'To must be greater than or equal to From.';
	return null;
}
