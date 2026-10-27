// The busy notice of a held tracker write, on the toast stack: its words,
// naming the tracker by its display name, its live countdown and what a
// screen reader hears of it; and the report of a progress write that
// could not be saved. Split from `busy-save.ts`, which re-exports it, so
// each file stays inside the CRAP gate's per-file bar.

import type { Provider } from './types';
import { startCountdown } from './busy-countdown';
import { toastStore } from '$lib/toasts/store.svelte';
import { m } from '$lib/paraglide/messages';

/** A tracker's display name; the provider id for one without a name. */
export function providerName(provider: Provider): string {
	if (provider === 'anilist') return m.account_provider_anilist();
	if (provider === 'mal') return m.account_provider_mal();
	return provider;
}

/** The notice's words with `seconds` left: the wait, or once it is over,
 *  that the save is going. */
function busyText(provider: Provider, seconds: number): string {
	const name = providerName(provider);
	return seconds > 0
		? m.account_busy_saving({ provider: name, seconds })
		: m.account_busy_saving_now({ provider: name });
}

/** A real one-shot timer, wrapped in arrows: `setTimeout` called off an
 *  object is an "Illegal invocation" in Chromium. */
export function after(ms: number, tick: () => void): () => void {
	const handle = setTimeout(() => tick(), ms);
	return () => clearTimeout(handle);
}

/** The clock a held write's wait and its notice are measured on: one
 *  that runs on steadily, whatever is done to the wall clock. */
export function monotonic(): number {
	return performance.now();
}

/** The toast shown while `provider` waits `seconds` to take a write. It
 *  counts the wait down in place, read from one deadline, and then says
 *  the save is going; it stays up until taken down — by the write it is
 *  about, a newer write, or the user — and its timer stops whenever it
 *  leaves. Returns its id. */
export function noticeBusy(provider: Provider, seconds: number): string {
	let stop: (() => void) | null = null;
	const id = toastStore.push({
		kind: 'info',
		message: busyText(provider, seconds),
		duration: null,
		onClose: () => stop?.()
	});
	stop = startCountdown(Date.now() + seconds * 1000, {
		now: () => Date.now(),
		after,
		// The number changes on screen each second without being read out;
		// the save going is announced.
		show: (left) => toastStore.update(id, busyText(provider, left), { silent: left > 0 })
	});
	return id;
}

/** The error toast for a progress write that could not be saved. */
export function reportProgressNotSaved(provider: Provider): void {
	toastStore.push({
		kind: 'error',
		message: m.account_progress_save_failed({ provider: providerName(provider) })
	});
}
