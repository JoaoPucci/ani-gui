/**
 * Whether the notification center runs this launch.
 *
 * It sits behind the `notification_center` launch flag and ships off:
 * only the flag set to `true` turns it on, which the backend allows
 * only from a flags file whose signature verifies (a rollout,
 * backend/src/notices/serve.rs). Like every flag it is read
 * once, from the flags the window was opened with, so it never changes
 * mid-session. Off, the app is what it was before the center existed:
 * no bell, no toasts or rows from it, no detail page, no line above
 * Play, and nothing read from or written to its storage.
 */
import { launchFlags, type LaunchFlags } from '$lib/flags';

export const CENTER_FLAG = 'notification_center';

export function centerEnabled(flags: LaunchFlags = launchFlags()): boolean {
	return flags[CENTER_FLAG] === true;
}
