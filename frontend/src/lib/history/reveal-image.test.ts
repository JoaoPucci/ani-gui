import { describe, expect, it, vi } from 'vitest';
import { revealWhenDecoded, type RevealTarget } from './reveal-image';

/** An image element as far as the action can tell. */
function fakeImage(
	state: { complete?: boolean; naturalWidth?: number; decode?: () => Promise<void> } = {}
) {
	const events = new EventTarget();
	const img: RevealTarget & { fire(type: 'load' | 'error'): void } = {
		complete: state.complete ?? false,
		naturalWidth: state.naturalWidth ?? 0,
		dataset: {},
		decode: state.decode,
		addEventListener: (t, fn) => events.addEventListener(t, fn),
		removeEventListener: (t, fn) => events.removeEventListener(t, fn),
		fire: (t) => events.dispatchEvent(new Event(t))
	};
	return img;
}

const settle = () => new Promise((r) => setTimeout(r, 0));

describe('revealWhenDecoded', () => {
	it('keeps the image hidden until it has loaded and decoded, then fades it in', async () => {
		let decoded: () => void = () => {};
		const img = fakeImage({ decode: () => new Promise<void>((r) => (decoded = r)) });
		revealWhenDecoded(img);
		expect(img.dataset.revealed).toBeUndefined();
		img.fire('load');
		await settle();
		expect(img.dataset.revealed).toBeUndefined();
		decoded();
		await settle();
		expect(img.dataset.revealed).toBe('fade');
	});

	it('shows an image the browser already holds at once, without a fade', async () => {
		const img = fakeImage({ complete: true, naturalWidth: 320, decode: () => Promise.resolve() });
		revealWhenDecoded(img);
		expect(img.dataset.revealed).toBe('instant');
		img.fire('load');
		await settle();
		expect(img.dataset.revealed).toBe('instant');
	});

	it('waits for an element that is complete with no pixels yet', async () => {
		const img = fakeImage({ complete: true, naturalWidth: 0 });
		const onError = vi.fn();
		revealWhenDecoded(img, { onError });
		expect(img.dataset.revealed).toBeUndefined();
		expect(onError).not.toHaveBeenCalled();
		img.fire('load');
		await settle();
		expect(img.dataset.revealed).toBe('fade');
	});

	it('shows a loaded image whose decode is refused', async () => {
		const img = fakeImage({ decode: () => Promise.reject(new Error('detached')) });
		revealWhenDecoded(img);
		img.fire('load');
		await settle();
		expect(img.dataset.revealed).toBe('fade');
	});

	it('reports a failed load and leaves the image hidden', () => {
		const img = fakeImage();
		const onError = vi.fn();
		revealWhenDecoded(img, { onError });
		img.fire('error');
		expect(onError).toHaveBeenCalledTimes(1);
		expect(img.dataset.revealed).toBeUndefined();
	});

	it('does nothing once the card is gone', async () => {
		const img = fakeImage({ decode: () => Promise.resolve() });
		const onError = vi.fn();
		const action = revealWhenDecoded(img, { onError });
		img.fire('load');
		action.destroy();
		await settle();
		img.fire('error');
		expect(img.dataset.revealed).toBeUndefined();
		expect(onError).not.toHaveBeenCalled();
	});
});
