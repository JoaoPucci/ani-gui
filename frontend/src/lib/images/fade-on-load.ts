/**
 * A Svelte action for images that fade in once their pixels arrive.
 * Not used yet: it marks every image loaded at once.
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

/** Mark `img` loaded. */
export function fadeOnLoad(img: FadingImage): { destroy(): void } {
	img.classList.add(LOADED_CLASS);
	return { destroy() {} };
}
