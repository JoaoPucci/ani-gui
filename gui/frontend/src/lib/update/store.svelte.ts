/**
 * Singleton state for the update notifier.
 *
 * Holds the latest GitHub release (when newer than current) plus a
 * "user has acknowledged this version" flag, persisted in
 * localStorage so the neon glow doesn't reappear on every launch
 * once the user has seen the dialog for a given tag.
 *
 * Glow visibility:
 *   - available && dismissedTag !== available.tag → glow ON
 *     (user hasn't acknowledged THIS tag yet)
 *   - available && dismissedTag === available.tag → glow OFF
 *     (badge still visible — version is still newer than current —
 *     but the user has already seen the dialog, so no pulse)
 *
 * A future release bumps `available.tag`, which makes
 * `dismissedTag !== available.tag` flip back to true, so the glow
 * reappears for the new version without us having to clear state.
 */

import type { ReleaseInfo } from './release-parse';

const DISMISSED_KEY = 'ani-gui-update-dismissed-tag';

function readDismissed(): string | null {
	try {
		return typeof localStorage !== 'undefined' ? localStorage.getItem(DISMISSED_KEY) : null;
	} catch {
		return null;
	}
}

function writeDismissed(tag: string): void {
	try {
		localStorage.setItem(DISMISSED_KEY, tag);
	} catch {
		// Storage quota / disabled — accept; in-memory state still
		// updates, the user just gets the glow back next launch.
	}
}

class UpdateStore {
	available = $state<ReleaseInfo | null>(null);
	dismissedTag = $state<string | null>(readDismissed());
	/** Modal dialog open state — separate from dismissed so the user
	 *  can dismiss the glow and still re-open the dialog from the
	 *  badge afterward. */
	dialogOpen = $state(false);

	readonly hasUpdate = $derived(this.available !== null);
	readonly glowing = $derived(this.available !== null && this.dismissedTag !== this.available.tag);

	setAvailable(release: ReleaseInfo | null): void {
		this.available = release;
	}

	openDialog(): void {
		this.dialogOpen = true;
		// Clicking the badge counts as acknowledgement — stop the glow
		// for this tag. The dialog stays open until the user closes it.
		if (this.available) {
			this.dismissedTag = this.available.tag;
			writeDismissed(this.available.tag);
		}
	}

	closeDialog(): void {
		this.dialogOpen = false;
	}
}

export const updateStore = new UpdateStore();
