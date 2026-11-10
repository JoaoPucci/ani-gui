/**
 * What the modal layer ($lib/components/modal-layer) does about what
 * the browser shows above the page: fullscreen, and a <dialog> shown
 * modally in the top layer — split from it for the per-file complexity
 * bar.
 */

/** A modal the user cannot see would hold a keyboard they cannot use:
 *  a modal opening outside the fullscreen element leaves fullscreen,
 *  all but a <dialog>, which the app only shows with showModal() and
 *  the top layer shows above fullscreen. */
export function leaveHidingFullscreen(doc: Document, root: HTMLElement): void {
	// A <dialog>, which the app only shows modally, sits in the top layer,
	// above fullscreen, and is seen there.
	if (root instanceof HTMLDialogElement) return;
	const full = doc.fullscreenElement;
	if (full && !full.contains(root)) doc.exitFullscreen().catch(() => {});
}

/** A native dialog shown modally sits in the top layer, above every
 *  other modal: it steps out of it while one opens above it, and comes
 *  back once that one closes. */
export function stepAside(below: HTMLElement | undefined): void {
	if (below instanceof HTMLDialogElement && below.open) below.close();
}
export function stepBack(root: HTMLElement): void {
	if (root instanceof HTMLDialogElement && !root.open) root.showModal();
}
