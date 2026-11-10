/**
 * A modal holds the keyboard while it is up, the way a well-made app's
 * dialogs do:
 *
 * - it takes focus as it opens: its control marked `data-autofocus`,
 *   else its first control, else its own surface;
 * - Tab and Shift+Tab cycle inside it ($lib/components/focus-cycle);
 * - the rest of the page is `inert` ($lib/components/page-inert), out
 *   of reach of the keyboard, the pointer and assistive tech alike —
 *   including whatever mounts beside it while it is up;
 * - Escape does what its close does, when it is given one;
 * - closing it gives focus back to what held it when it opened, if that
 *   is still on the page.
 *
 * While one is up, `isModalOpen()` is true, and the page's own
 * window-level shortcuts stand down.
 *
 * Modals stack: only the topmost one is live, and closing it hands the
 * page back to the one beneath. A modal that closes from under another
 * passes its return target up, so focus still ends where the first of
 * them was opened from — a resume's loading overlay giving way to its
 * failure, say.
 *
 * A modal the user cannot see would hold a keyboard they cannot use: a
 * page element in fullscreen hides everything outside it, so a modal
 * opening outside the fullscreen element leaves fullscreen first — all
 * but a native <dialog> shown modally, which the top layer shows above
 * fullscreen (the change-show picker over a fullscreen player).
 */

import { nextInCycle, tabbablesIn } from './focus-cycle';
import { holdOutside } from './page-inert';

export interface ModalOptions {
	/** What Escape does while this is the topmost modal. Without it,
	 *  Escape is left alone. */
	onEscape?: () => void;
}

export interface ModalHandle {
	update(opts: ModalOptions): void;
	close(): void;
}

interface Layer {
	root: HTMLElement;
	opts: ModalOptions;
	returnTo: HTMLElement | null;
}

const stack: Layer[] = [];

const top = (): Layer | undefined => stack[stack.length - 1];

function enter(root: HTMLElement): void {
	const inside = tabbablesIn(root);
	(inside.find((el) => el.hasAttribute('data-autofocus')) ?? inside[0] ?? root).focus();
}

// Listening only while a modal is up, so there is always a topmost.
function onKeydown(e: KeyboardEvent): void {
	const layer = top()!;
	if (e.key === 'Tab') {
		e.preventDefault();
		const active = layer.root.ownerDocument.activeElement as HTMLElement | null;
		(nextInCycle(tabbablesIn(layer.root), active, e.shiftKey) ?? layer.root).focus();
	} else if (e.key === 'Escape' && layer.opts.onEscape) {
		e.preventDefault();
		e.stopPropagation();
		layer.opts.onEscape();
	}
}

function leaveHidingFullscreen(doc: Document, root: HTMLElement): void {
	// A native dialog the page shows modally sits in the top layer, above
	// fullscreen, and is seen there.
	if (root instanceof HTMLDialogElement) return;
	const full = doc.fullscreenElement;
	if (full && !full.contains(root)) doc.exitFullscreen().catch(() => {});
}

function close(layer: Layer): void {
	const at = stack.indexOf(layer);
	if (at === -1) return;
	stack.splice(at, 1);
	for (const above of stack.slice(at)) {
		if (layer.root.contains(above.returnTo)) above.returnTo = layer.returnTo;
	}
	holdOutside(top()?.root ?? null);
	if (stack.length === 0) layer.root.ownerDocument.removeEventListener('keydown', onKeydown, true);
	// Only the topmost modal held focus; one closing beneath another
	// leaves it where it is.
	if (at < stack.length) return;
	// A return target that is a modal's own surface (one opened with
	// focus on nothing) is entered like a modal: at its control.
	const back = layer.returnTo;
	if (back?.isConnected && !stack.some((l) => l.root === back)) back.focus();
	else if (top()) enter(top()!.root);
}

/** Whether a modal holds the page. A page's own window-level
 *  shortcuts — the player's, the topbar's — stand down while it does:
 *  the modal is the only keyboard target. */
export function isModalOpen(): boolean {
	return stack.length > 0;
}

/** Make `root` the topmost modal on its page. */
export function openModal(root: HTMLElement, opts: ModalOptions = {}): ModalHandle {
	const doc = root.ownerDocument;
	const active = doc.activeElement;
	// Focus on nothing as a modal opens over another (a busy confirm's
	// disabled button drops it) returns into the one beneath.
	const held = active instanceof HTMLElement && active !== doc.body ? active : null;
	const layer: Layer = { root, opts, returnTo: held ?? top()?.root ?? null };
	if (stack.length === 0) doc.addEventListener('keydown', onKeydown, true);
	stack.push(layer);
	leaveHidingFullscreen(doc, root);
	holdOutside(root);
	enter(root);
	return {
		update: (next) => (layer.opts = next),
		close: () => close(layer)
	};
}

/** `use:modal={{ onEscape }}` — the element is a modal for as long as
 *  it is mounted. */
export function modal(node: HTMLElement, opts: ModalOptions = {}) {
	const handle = openModal(node, opts);
	return { update: handle.update, destroy: handle.close };
}
