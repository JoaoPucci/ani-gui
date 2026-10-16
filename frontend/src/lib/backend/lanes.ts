/**
 * The backend's lanes: the loopback origins it listens on, one per
 * class of renderer traffic (see the backend's `lanes` module).
 */

/** Every lane the backend announces. */
export const LANES = ['api', 'stream', 'image', 'probe', 'resolve', 'download'] as const;

export type Lane = (typeof LANES)[number];

/** The origin `lane` is reached at. */
export function laneBase(
	_lanes: Partial<Record<string, string>> | undefined,
	_lane: Lane,
	apiBase: string
): string {
	return apiBase.replace(/\/+$/, '');
}
