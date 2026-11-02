// Shows a card's artwork only once it can be drawn whole.
//
// An image painted as it streams in arrives as a band sweeping down the
// tile. The card instead keeps its skeleton until the image has loaded
// and decoded, then marks it revealed; the stylesheet fades it in. An
// image the browser already holds is revealed at once and does not
// fade, so a card whose artwork is in memory draws as it was left.

/** The parts of an image element this needs; an `HTMLImageElement` fits. */
export interface RevealTarget {
	readonly complete: boolean;
	readonly naturalWidth: number;
	readonly dataset: DOMStringMap;
	decode?: () => Promise<void>;
	addEventListener(type: 'load' | 'error', fn: () => void): void;
	removeEventListener(type: 'load' | 'error', fn: () => void): void;
}

export interface RevealOptions {
	/** The image failed to load; the card moves on to its fallback. */
	onError?: () => void;
}

/**
 * Svelte action: sets `data-revealed` to `instant` (the image was
 * already there) or `fade` (it arrived after the card drew) once the
 * image is ready to paint, and reports a failed load.
 */
export function revealWhenDecoded(img: RevealTarget, options: RevealOptions = {}) {
	let live = true;
	const reveal = (how: 'instant' | 'fade') => {
		if (live && !img.dataset.revealed) img.dataset.revealed = how;
	};
	const fail = () => {
		if (live) options.onError?.();
	};
	const onLoad = () => {
		// A decode can reject for reasons that are not a broken image
		// (the element left the document, say); the load already
		// succeeded, so the image is shown either way.
		const decoded = img.decode ? img.decode() : Promise.resolve();
		void decoded.then(
			() => reveal('fade'),
			() => reveal('fade')
		);
	};
	// Listened for even when the image is already there: the browser
	// still dispatches its load (or error) as a later task, and an
	// element that reports `complete` with no pixels may be one whose
	// request has not started yet rather than a broken one.
	img.addEventListener('load', onLoad);
	img.addEventListener('error', fail);
	if (img.complete && img.naturalWidth > 0) reveal('instant');
	return {
		update(next: RevealOptions) {
			void next;
		},
		destroy() {
			live = false;
			img.removeEventListener('load', onLoad);
			img.removeEventListener('error', fail);
		}
	};
}
