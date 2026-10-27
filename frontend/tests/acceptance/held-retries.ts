// The held retries the play page arms, seen through the timer module
// that runs them.
//
// A case that asserts a held retry did not start the engine has to
// know the retry is no longer able to. Waiting a fixed time past the
// hold's delay assumes the retry is due by then; a retry armed with a
// longer delay than the case expected fires after the wait and the
// negative holds without having checked anything. Here every retry
// the page arms is recorded, and a case waits until each has fired or
// been cancelled — after which nothing a retry does is still to come.
//
// Install it from a case file with
//
//   vi.mock('../../src/lib/play/held-retry', async (importOriginal) =>
//     (await import('./held-retries')).trackHeldRetries(await importOriginal())
//   );
//
// and clear `heldRetries` in `beforeEach`.

import type * as HeldRetryModule from '../../src/lib/play/held-retry';

export type HeldRetryState = 'pending' | 'fired' | 'cancelled';

/** Every retry armed in the current case, in arming order. */
export const heldRetries: { state: HeldRetryState }[] = [];

/** Whether every retry armed so far has fired or been cancelled. True
 *  when none was armed, so it witnesses only retries the page arms
 *  synchronously with the failure; a case expecting none says so by
 *  asserting `heldRetries` empty. */
export const heldRetriesSettled = () => heldRetries.every((r) => r.state !== 'pending');

export function trackHeldRetries(real: typeof HeldRetryModule): typeof HeldRetryModule {
	return {
		...real,
		scheduleHeldRetry(retry) {
			const record: { state: HeldRetryState } = { state: 'pending' };
			heldRetries.push(record);
			const cancel = real.scheduleHeldRetry({
				...retry,
				fire: () => {
					if (record.state === 'pending') record.state = 'fired';
					retry.fire();
				}
			});
			return () => {
				if (record.state === 'pending') record.state = 'cancelled';
				cancel();
			};
		}
	};
}
