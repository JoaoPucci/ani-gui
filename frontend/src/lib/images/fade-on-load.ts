/**
 * A Svelte action for images that fade in once their pixels arrive —
 * an episode still on a show's page or in the player, including one a
 * held-back fill brings later. A Continue card's artwork has its own
 * reveal, `$lib/history/reveal-image`.
 *
 * The fade starts on `load`, not when the element is inserted: an
 * animation run on insertion is usually spent before a cold image's
 * bytes arrive, and the image then pops in anyway. An image already
 * decoded when the action attaches is shown at once, and one that fails
 * is not left hidden. The element stays when its source changes, so an
 * image already on screen stays up until the next is ready. The opacity
 * rule lives in `tokens.css` (`.img-fade`); under reduced motion its
 * duration is zero.
 */

/** The class an image carries once it may be seen. */
export const LOADED_CLASS = 'img-loaded';

/** What the action needs of an image element. */
export interface FadingImage {
	complete: boolean;
	naturalWidth: number;
	classList: { add(name: string): void };
	addEventListener(name: string, fn: () => void): void;
	removeEventListener(name: string, fn: () => void): void;
}

/** Show `img` once it has loaded (or failed); at once if it already has. */
export function fadeOnLoad(img: FadingImage): { destroy(): void } {
	const shown = () => img.classList.add(LOADED_CLASS);
	if (img.complete && img.naturalWidth > 0) shown();
	img.addEventListener('load', shown);
	img.addEventListener('error', shown);
	return {
		destroy() {
			img.removeEventListener('load', shown);
			img.removeEventListener('error', shown);
		}
	};
}
