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
	/** Runs `action` for a press of the dialog's confirm button. A press
	 *  while an action is out starts nothing and settles with it. A
	 *  failure is handed to `failed` before the dialog closes, so the
	 *  message is up as focus goes back to the control that opened the
	 *  dialog; it never escapes, and the returned promise always
	 *  resolves. */
	confirm(action: () => Promise<unknown>, failed: (error: unknown) => void): Promise<void>;
	/** Opens the dialog, through `show`. */
	open(show: () => void): void;
	/** A press of Cancel, Esc or the backdrop. Nothing while an action
	 *  is out: closing then would hide how it ends. */
	cancel(): void;
}

export function createConfirmRun(deps: ConfirmRunDeps): ConfirmRun {
	let inFlight: Promise<void> | null = null;
	return {
		get busy() {
			return inFlight !== null;
		},
		confirm(action, failed) {
			if (inFlight) return inFlight;
			deps.setBusy(true);
			inFlight = started(action)
				.then(
					() => {},
					(error) => report(failed, error)
				)
				.finally(() => {
					inFlight = null;
					deps.setBusy(false);
					deps.close();
				});
			return inFlight;
		},
		open(show) {
			show();
		},
		cancel() {
			if (inFlight) return;
			deps.close();
		}
	};
}

/** The action, called now; one that throws as it is called is a
 *  rejected one, so the run settles after `confirm` has noted it. */
function started(action: () => Promise<unknown>): Promise<unknown> {
	try {
		return Promise.resolve(action());
	} catch (error) {
		return Promise.reject(error);
	}
}

function report(failed: (error: unknown) => void, error: unknown): void {
	try {
		failed(error);
	} catch {
		// A report that breaks must not keep the dialog open.
	}
}
