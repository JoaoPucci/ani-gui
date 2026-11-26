/**
 * The Debug page's Seen toggles ($lib/diagnostics/tip-keys reads and
 * names the list): one change at a time, every toggle held while it is
 * out; a change the backend refuses is undone on the box and said; the
 * list read again afterwards, and kept as it was when that read fails;
 * the session's tips told, so the next page follows. Rendering glue:
 * the toggles on screen.
 */
import type { OnboardingDebugTip, OnboardingDebugView } from '$lib/api';
import { readTipKeys, type TipKeysState } from './tip-keys';

export interface ToggleDeps {
	/** Send the change. */
	send: (id: string, seen: boolean) => Promise<void>;
	/** Read the list again. */
	read: () => Promise<OnboardingDebugView>;
	/** Tell the session's tips. */
	tell: (id: string, seen: boolean) => void;
}

export class TipSeenToggles {
	/** The list as last read. */
	state = $state<TipKeysState>({ kind: 'loading' });
	/** A change is out: every toggle waits. */
	busy = $state(false);
	/** The last change, or the read after it, failed. */
	failed = $state(false);
	readonly #deps: ToggleDeps;

	constructor(deps: ToggleDeps) {
		this.#deps = deps;
	}

	/** Read the list. */
	async load(): Promise<void> {
		this.state = await readTipKeys(this.#deps.read);
	}

	/** The box for `tip` was set to `box.checked`. */
	async set(tip: OnboardingDebugTip, box: { checked: boolean }): Promise<void> {
		if (this.busy) return;
		const want = box.checked;
		this.busy = true;
		this.failed = false;
		try {
			await this.#deps.send(tip.id, want);
			this.#deps.tell(tip.id, want);
		} catch {
			box.checked = tip.seen;
			this.failed = true;
		}
		const again = await readTipKeys(this.#deps.read);
		if (again.kind === 'read') this.state = again;
		else this.failed = true;
		this.busy = false;
	}
}
