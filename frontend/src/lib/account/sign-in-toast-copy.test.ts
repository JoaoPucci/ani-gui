import { describe, expect, it } from 'vitest';
import { signInToastCopy } from './sign-in-toast-copy';
import type { SignInNotice } from './expiry-toast';

describe('signInToastCopy', () => {
	it.each<[SignInNotice, string, string, 'reconnect' | 'retry']>([
		['expired', 'Your MAL session expired. Sign in again to keep syncing.', 'Sign in', 'reconnect'],
		[
			'refused',
			'MAL refused to renew your sign-in. Reconnect to keep syncing.',
			'Reconnect',
			'reconnect'
		],
		[
			'unreachable',
			"Couldn't reach MAL to renew your sign-in. It may still be valid; the app keeps trying.",
			'Try now',
			'retry'
		],
		[
			'unknown',
			"Couldn't renew your MAL sign-in. Try again, or reconnect if it keeps failing.",
			'Try again',
			'retry'
		]
	])('says what is known for a %s sign-in', (notice, message, actionLabel, action) => {
		expect(signInToastCopy(notice, 'MAL')).toEqual({ message, actionLabel, action });
	});

	it('asks to reconnect only when the tracker refused or the session ran out', () => {
		const reconnecting = (['expired', 'refused', 'unreachable', 'unknown'] as const).filter(
			(n) => signInToastCopy(n, 'MAL').action === 'reconnect'
		);
		expect(reconnecting).toEqual(['expired', 'refused']);
	});
});
