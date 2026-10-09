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

import { m } from '$lib/paraglide/messages';

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
		return m.download_range_error_not_a_number();
	}
	if (s < 1 || e < 1) return m.download_range_error_below_one();
	if (s > rangeMax) {
		return maxEpisode
			? m.download_range_error_start_over_max({ max: String(maxEpisode) })
			: m.download_range_error_over_cap({ max: String(rangeMax) });
	}
	if (e > rangeMax) {
		return maxEpisode
			? m.download_range_error_end_over_max({ max: String(maxEpisode) })
			: m.download_range_error_over_cap({ max: String(rangeMax) });
	}
	if (e < s) return m.download_range_error_end_before_start();
	return null;
}
