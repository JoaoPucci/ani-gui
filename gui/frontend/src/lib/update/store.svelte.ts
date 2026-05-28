/**
 * Singleton state for the update notifier.
 *
 * Holds the latest GitHub release (when newer than current) plus
 * the modal-open flag. The LED pulse on `<UpdateBadge>` is
 * deliberately persistent for as long as a newer release exists —
 * clicking the badge opens the dialog but does NOT retire the
 * pulse, because users want an ambient reminder on every launch
 * until they actually upgrade. Once `currentVersion` catches up,
 * `checkForUpdate` returns `null`, `available` clears, and the
 * pulse goes away naturally.
 */

import type { ReleaseInfo } from './release-parse';

class UpdateStore {
	available = $state<ReleaseInfo | null>(null);
	dialogOpen = $state(false);

	readonly hasUpdate = $derived(this.available !== null);

	setAvailable(release: ReleaseInfo | null): void {
		this.available = release;
	}

	openDialog(): void {
		this.dialogOpen = true;
	}

	closeDialog(): void {
		this.dialogOpen = false;
	}
}

export const updateStore = new UpdateStore();
