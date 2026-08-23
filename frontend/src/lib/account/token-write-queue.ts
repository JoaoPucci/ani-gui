/**
 * Per-provider FIFO serializer for safeStorage token mutations.
 *
 * `persistAccount` (setToken) and `clearPersistedAccount` (clearToken)
 * travel on separate Electron IPC channels. The main process gives no
 * cross-channel ordering guarantee, so a boot-time refresh's persist
 * could commit *after* a racing disconnect's clear and resurrect on
 * disk a token the user just removed — the next `hydrate()` then
 * reconnects the account (Codex P2 #3416883099). The renderer's
 * generation re-check (`refresh-flow.ts`) only suppresses the in-memory
 * store update; it cannot undo a disk write that already landed.
 *
 * Chaining every write of a given provider onto a single promise makes
 * the on-disk order match the renderer's (single-threaded) call order:
 * the next IPC is not sent until the previous one has settled, so a
 * disconnect's clear enqueued after a refresh's persist always wins.
 * Combined with the synchronous `beginAccountChange` + pre-persist
 * generation check, a doomed refresh write is never enqueued in the
 * first place.
 *
 * Queues are keyed per provider so unrelated providers still write
 * concurrently. Failures are isolated: a rejected op settles the chain
 * (swallowed for the *next* op) without poisoning it, while the
 * original caller still sees the rejection.
 */

import type { Provider } from './types';

const chains: Partial<Record<Provider, Promise<unknown>>> = {};

export function enqueueTokenWrite<T>(provider: Provider, op: () => Promise<T>): Promise<T> {
	const prev = chains[provider] ?? Promise.resolve();
	// Run `op` once `prev` settles, regardless of whether it fulfilled or
	// rejected (both handlers are `op`), so one failed write can't stall
	// the queue.
	const next = prev.then(op, op);
	// Keep the chain alive on a rejection-swallowed continuation so a
	// failed op doesn't reject the *next* enqueue's `prev`; the caller's
	// `next` still carries the real outcome.
	chains[provider] = next.then(
		() => undefined,
		() => undefined
	);
	return next;
}

/** Test-only: clear all per-provider chains between cases. */
export function __resetTokenWriteQueues(): void {
	for (const key of Object.keys(chains) as Provider[]) {
		delete chains[key];
	}
}
