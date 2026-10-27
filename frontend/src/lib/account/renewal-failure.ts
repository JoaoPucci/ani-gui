/**
 * What a failed sign-in renewal says about the sign-in.
 *
 * A renewal can fail because the tracker refused the refresh token
 * (`rejected`: only a new sign-in helps), because the tracker did not
 * answer (`unreachable`: the same sign-in may still work), or for a
 * reason the app cannot tell (`unknown`). The copy a notice shows is
 * chosen from this, so a notice says "reconnect" only when the
 * tracker refused.
 */

export type RenewalFailure = 'rejected' | 'unreachable' | 'unknown';

/** Per-provider renewal failure, `null` while none is outstanding. */
export type RenewalFailures = Record<'anilist' | 'mal' | 'inhouse', RenewalFailure | null>;

export const NO_RENEWAL_FAILURES: RenewalFailures = { anilist: null, mal: null, inhouse: null };

/** Read a failed renewal call as one of the three. */
export function classifyRenewalError(_error: unknown): RenewalFailure {
	void _error;
	return 'unknown';
}

/** True when a read was refused because its bearer was. */
export function isBearerRefused(_error: unknown): boolean {
	void _error;
	return false;
}
