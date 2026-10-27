// Where a tracker's account stands against the user a write was listed
// under, read from plain values. Split from `held-account.ts`, which
// re-exports it and reads the store for it, so each file stays inside
// the CRAP gate's per-file bar.

/** Where a tracker's user `now` stands against the user a write was
 *  `listed` under: `gone` when either is unknown, else `same` or
 *  `other`. */
export function standingOf(
	listed: string | null | undefined,
	now: string | null | undefined
): 'same' | 'other' | 'gone' {
	if (listed === null || listed === undefined || now === null || now === undefined) return 'gone';
	return now === listed ? 'same' : 'other';
}

/** Where a tracker's account is now, as `hasLeft` reads it. */
export interface AccountNow {
	/** An account change is under way. */
	changing: boolean;
	/** A sign-in is under way. */
	connecting: boolean;
	/** The tracker is disconnected. */
	disconnected: boolean;
	/** The user its account names, if it names one. */
	user: string | null | undefined;
}

/**
 * Whether a tracker has for good left the user a write was listed under
 * — so a held write need not wait for its end-of-wait check to know it
 * is dropped. Disconnected, or another user's account, is final; a
 * sign-in or an account change under way is not, since either can end
 * on the same user. A write listed under no user was never that user's.
 */
export function hasLeft(listedUser: string | null | undefined, now: AccountNow): boolean {
	if (listedUser === null || listedUser === undefined) return true;
	if (now.changing || now.connecting) return false;
	return now.disconnected || now.user !== listedUser;
}
