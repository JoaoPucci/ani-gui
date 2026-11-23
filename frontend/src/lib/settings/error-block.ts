/**
 * The Settings page's error block: which action's failure it shows.
 *
 * A save, Clear history and Clear image cache each report a failure in
 * the same block, headed by the action that failed. The block shows the
 * last failure until its own action lands: a save that lands does not
 * take down a clear's failure, since nothing was cleared, and a clear
 * that lands does not take down a save's.
 */

export type ErrorAction = 'save' | 'clear_history' | 'clear_images';

/** What the block shows, or null when it is down. */
export type ErrorBlock = { action: ErrorAction; detail: string } | null;

/** `action` failed with `detail`: the block is now about it. */
export function failed(_block: ErrorBlock, action: ErrorAction, detail: string): ErrorBlock {
	return { action, detail };
}

/** `action` landed: the block comes down if it was about that action. */
export function landed(block: ErrorBlock, action: ErrorAction): ErrorBlock {
	return block?.action === action ? null : block;
}
