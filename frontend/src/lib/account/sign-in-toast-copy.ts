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

export function signInToastCopy(_notice: SignInNotice, provider: string): SignInToastCopy {
	void _notice;
	return {
		message: m.account_expiry_toast_message({ provider }),
		actionLabel: m.account_expiry_toast_action(),
		action: 'reconnect'
	};
}
