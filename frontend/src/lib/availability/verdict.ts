/**
 * What an availability answer establishes, read in one place.
 *
 * A settled answer is a verdict: `available` says whether the provider
 * carries the show. An unsettled one reached the provider and came
 * back without one; its `available: false` means nothing, and every
 * surface that gates Play or Download on availability reads it as the
 * unknown it is — the card stays, the page keeps its lazy play — as
 * it reads a failed request. One reason is the exception: `refused`
 * says the catalogue answered and lists nothing named for the show,
 * so Play can only fail, and it reads as the show not being in the
 * catalogue. The backend persists nothing for it either way, so the
 * next look asks again.
 */

import type { AvailabilityResponse } from '$lib/api';

/** Whether the catalogue refused the show by name. */
export function refusedByName(r: Pick<AvailabilityResponse, 'unsettled'>): boolean {
	return r.unsettled === 'refused';
}

/** Whether the answer establishes nothing about the show: its
 *  `available` is no verdict, and the reader keeps what it had for
 *  that show and audio ($lib/availability/row). True
 *  for every unsettled answer but a refusal by name — `true` from a
 *  backend before reasons included. */
export function establishesNothing(r: Pick<AvailabilityResponse, 'unsettled'>): boolean {
	return Boolean(r.unsettled) && !refusedByName(r);
}

/** The verdict the answer gives on whether the provider carries the
 *  show, or null when it gives none ({@link establishesNothing}). */
export function availabilityVerdict(
	r: Pick<AvailabilityResponse, 'available' | 'unsettled'>
): boolean | null {
	if (refusedByName(r)) return false;
	return establishesNothing(r) ? null : r.available;
}
