/**
 * The run of a confirmed destructive action on Home: Clear history, or
 * removing one Continue card. The confirm dialog shows the run as busy
 * while the action is out, and closes once it settles.
 *
 * The component holds the dialog's open and busy state; this module
 * holds the run around them, so it can be tested without mounting
 * the page.
 */

export interface ConfirmRunDeps {
	/** Shows the dialog's buttons as busy, or ready again. */
	setBusy(busy: boolean): void;
	/** Closes the dialog. */
	close(): void;
}

export interface ConfirmRun {
	/** Whether an action is out. */
	readonly busy: boolean;
	/** Runs `action` for a press of the dialog's confirm button. */
	confirm(action: () => Promise<unknown>, failed: (error: unknown) => void): Promise<void>;
	/** A press of Cancel, Esc or the backdrop. */
	cancel(): void;
}

export function createConfirmRun(deps: ConfirmRunDeps): ConfirmRun {
	return {
		busy: false,
		async confirm(action) {
			deps.setBusy(true);
			try {
				await action();
			} finally {
				deps.setBusy(false);
				deps.close();
			}
		},
		cancel() {
			deps.close();
		}
	};
}
