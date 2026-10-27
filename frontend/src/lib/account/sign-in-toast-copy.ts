/**
 * The words and the action of a sign-in notice. The layout pushes the
 * toast; this decides what it says, so the copy for each notice has a
 * unit test of its own.
 */

import { m } from '$lib/paraglide/messages';
import type { SignInNotice } from './expiry-toast';

export interface SignInToastCopy {
	message: string;
	actionLabel: string;
	/** What the action does: open the account page to sign in again,
	 *  or try the renewal again now. */
	action: 'reconnect' | 'retry';
}

/** Each notice's copy says only what is known: "reconnect" when the
 *  session ran out with no way to renew it or the tracker refused the
 *  renewal; "could not reach" when the tracker did not answer, since
 *  the sign-in may still be valid; and no cause when none is known. */
export function signInToastCopy(notice: SignInNotice, provider: string): SignInToastCopy {
	switch (notice) {
		case 'expired':
			return {
				message: m.account_expiry_toast_message({ provider }),
				actionLabel: m.account_expiry_toast_action(),
				action: 'reconnect'
			};
		case 'refused':
			return {
				message: m.account_signin_refused_message({ provider }),
				actionLabel: m.account_signin_refused_action(),
				action: 'reconnect'
			};
		case 'unreachable':
			return {
				message: m.account_signin_unreachable_message({ provider }),
				actionLabel: m.account_signin_unreachable_action(),
				action: 'retry'
			};
		case 'unknown':
			return {
				message: m.account_signin_unknown_message({ provider }),
				actionLabel: m.account_signin_unknown_action(),
				action: 'retry'
			};
	}
}
