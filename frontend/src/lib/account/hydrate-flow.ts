/**
 * `accountStore.hydrate()`'s reads: each provider's saved sign-in read
 * from the OS keyring and applied to the store as it answers. Split
 * from the store so the store stays inside the CRAP gate's per-file
 * bar.
 *
 * Asynchronous, provider by provider: a keyring that is locked or not
 * answering holds only its own reads, and the app renders signed out
 * until each answers. A read that answers after the provider's state
 * changed (a connect or disconnect while it waited) is dropped: what
 * the user did since is newer than what it read. Once that change has
 * landed, a provider it left signed out is read again (see
 * `readAfterChange`). A provider whose read
 * holds a different sign-in moves its generation, so a renewal still
 * out for the old one is superseded.
 */

import type { Provider, ProviderState } from './types';
import { changing, readOnce, waitFor, type HydrateTarget } from './account-read';
import type { KeyringWait } from './keyring-wait';

export type { HydrateTarget } from './account-read';

const PROVIDERS: Provider[] = ['anilist', 'mal', 'inhouse'];

/** One hydrate: whether its first reads have all settled, and its
 *  refresh run. */
interface HydrateRun {
	settled: boolean;
}

/** The provider's state kind, read now. */
function kindOf(target: HydrateTarget, p: Provider): ProviderState['kind'] {
	return target.byProvider[p].kind;
}

/**
 * After a change got in a read's way: wait until it has landed — read
 * while a connect or disconnect is under way, an answer would replace
 * the change's own state — and then read again only if it left the
 * provider signed out. That is a connect started and cancelled, where
 * the disk still holds the sign-in the overtaken read was after. A
 * change that ended anywhere else (signed in, an error it reported) has
 * said what the provider's state is. A sign-in found expired after the
 * hydrate's own refresh has run gets one of its own, for that provider.
 */
async function readAfterChange(
	target: HydrateTarget,
	p: Provider,
	wait: KeyringWait,
	hydrate: HydrateRun
): Promise<void> {
	for (;;) {
		while (changing(target, p)) await target.nextChange(p);
		if (kindOf(target, p) !== 'disconnected') return;
		if (await readOnce(target, p, wait)) {
			// While the hydrate is still out its own refresh covers this
			// sign-in; asking here too would exchange one refresh token twice.
			if (hydrate.settled && kindOf(target, p) === 'expired') void target.refreshExpired(p);
			return;
		}
	}
}

/**
 * The provider's first read. Settles once it has applied, or once a
 * change has got in its way — then the rest goes on in the background,
 * so a sign-in left open in the browser holds no other provider's
 * hydrate or refresh. The keyring notice counts the reads alone, not
 * the waits for a change.
 */
async function firstRead(
	target: HydrateTarget,
	p: Provider,
	wait: KeyringWait,
	hydrate: HydrateRun
): Promise<void> {
	if (!changing(target, p) && (await readOnce(target, p, wait))) return;
	void readAfterChange(target, p, wait, hydrate);
}

/**
 * Read every provider and apply each answer — a provider a change got in
 * the way of is read in the background (see `firstRead`) — then give a
 * just-expired but refreshable provider its refresh. Every hydrate path (cold launch
 * AND the account page) must do that, so the hydrate owns it rather
 * than each caller remembering.
 */
export function hydrateAccounts(target: HydrateTarget): Promise<void> {
	const wait = waitFor(target);
	const hydrate: HydrateRun = { settled: false };
	const settled = Promise.all(PROVIDERS.map((p) => firstRead(target, p, wait, hydrate)));
	return settled.then(() => {
		hydrate.settled = true;
		return target.refreshExpired();
	});
}
