/**
 * Whether the account store is waiting on the OS keyring, and the
 * notice that says so.
 *
 * Reading a saved sign-in asks the keyring to decrypt it, and a keyring
 * can take its time: a locked one waits on its unlock prompt, one never
 * set up on its "new keyring" prompt, an unresponsive secret service
 * until the bus gives up. The app renders signed out meanwhile and takes
 * the accounts when the keyring answers. A read still out after
 * `KEYRING_WAIT_MS` puts up a notice, so a signed-out home is not taken
 * for a lost sign-in; it comes down when the read settles.
 */

/** How long a keyring read runs before the notice goes up. */
export const KEYRING_WAIT_MS = 2000;

export interface KeyringWaitTimers {
	setTimer(fn: () => void, ms: number): unknown;
	clearTimer(handle: unknown): void;
}

/**
 * Counts the reads that have run past the wait. `onChange(true)` when
 * the first one does, `onChange(false)` when the last of them settles.
 */
export class KeyringWait {
	private waiting = 0;

	constructor(
		private readonly timers: KeyringWaitTimers,
		private readonly onChange: (waiting: boolean) => void,
		private readonly waitMs: number = KEYRING_WAIT_MS
	) {}

	/** Watch one read, settled when `settled` resolves or rejects. */
	track(settled: Promise<unknown>): void {
		let done = false;
		let counted = false;
		const handle = this.timers.setTimer(() => {
			if (done) return;
			counted = true;
			this.waiting += 1;
			if (this.waiting === 1) this.onChange(true);
		}, this.waitMs);
		const finish = () => {
			done = true;
			this.timers.clearTimer(handle);
			if (!counted) return;
			this.waiting -= 1;
			if (this.waiting === 0) this.onChange(false);
		};
		settled.then(finish, finish);
	}
}

export interface NoticeSink {
	push(): string;
	dismiss(id: string): void;
}

/** Keeps one notice up while `waiting`, and none otherwise. */
export class KeyringWaitNotice {
	private id: string | null = null;

	sync(waiting: boolean, sink: NoticeSink): void {
		if (waiting && this.id === null) this.id = sink.push();
		else if (!waiting && this.id !== null) {
			sink.dismiss(this.id);
			this.id = null;
		}
	}
}
