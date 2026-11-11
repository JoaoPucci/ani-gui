/**
 * The donation ask's toast (./donation-ask decides when it shows):
 * pushed when asked, and taken down when a video, fullscreen or the
 * player begins while it is up. Taken down, it counts as acted on, so
 * it does not come back. ./NoticeHost supplies the toast stack.
 */
import type { PushArgs } from '$lib/toasts/store.svelte';

export interface ToastPort {
	push(args: PushArgs): string;
	dismiss(id: string): void;
}

export interface AskToast {
	show(): void;
	/** The window became busy (`true`) or idle. */
	busy(now: boolean): void;
}

export function createAskToast(toasts: ToastPort, args: () => PushArgs): AskToast {
	let up: string | null = null;
	return {
		show() {
			const a = args();
			up = toasts.push({
				...a,
				onClose: () => {
					up = null;
					a.onClose?.();
				}
			});
		},
		busy(now) {
			if (now && up !== null) toasts.dismiss(up);
		}
	};
}
