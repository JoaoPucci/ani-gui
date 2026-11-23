import { describe, expect, it } from 'vitest';
import { fadeOnLoad, LOADED_CLASS, type FadingImage } from './fade-on-load';

/** An image as the action sees it: whether it is already decoded, its
 *  classes, and the events it would fire. */
function image(complete: boolean, naturalWidth = complete ? 640 : 0) {
	const classes = new Set<string>();
	const listeners: Record<string, (() => void)[]> = {};
	const img: FadingImage = {
		complete,
		naturalWidth,
		classList: {
			add: (c: string) => void classes.add(c)
		},
		addEventListener: (name: string, fn: () => void) => {
			(listeners[name] ??= []).push(fn);
		},
		removeEventListener: (name: string, fn: () => void) => {
			listeners[name] = (listeners[name] ?? []).filter((f) => f !== fn);
		}
	};
	const fire = (name: string) => (listeners[name] ?? []).forEach((f) => f());
	return { img, classes, fire, listeners };
}

describe('fadeOnLoad', () => {
	it('shows an image already decoded at once, without a fade', () => {
		const { img, classes } = image(true);
		fadeOnLoad(img);
		expect(classes.has(LOADED_CLASS)).toBe(true);
	});

	it('fades an image in only once its pixels have arrived', () => {
		const { img, classes, fire } = image(false);
		fadeOnLoad(img);
		expect(classes.has(LOADED_CLASS)).toBe(false);
		fire('load');
		expect(classes.has(LOADED_CLASS)).toBe(true);
	});

	it('does not leave a failed image hidden', () => {
		const { img, classes, fire } = image(false);
		fadeOnLoad(img);
		fire('error');
		expect(classes.has(LOADED_CLASS)).toBe(true);
	});

	it('stops listening once the image is gone', () => {
		const { img, listeners } = image(false);
		const action = fadeOnLoad(img);
		action.destroy();
		expect(listeners.load ?? []).toHaveLength(0);
		expect(listeners.error ?? []).toHaveLength(0);
	});
});
