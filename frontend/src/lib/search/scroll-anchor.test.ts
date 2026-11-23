// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cardPositions, scrollToAnchor } from './scroll-anchor';

/** A grid of cards laid out one per `rowHeight`, under a header
 *  `offset` tall. happy-dom has no layout, so each card carries the
 *  offsets a browser would give it. */
function grid(ids: string[], rowHeight = 300, offset = 100) {
	const root = document.createElement('div');
	const list = document.createElement('ul');
	root.appendChild(list);
	ids.forEach((id, i) => {
		const li = document.createElement('li');
		li.dataset.cardId = id;
		Object.defineProperty(li, 'offsetTop', { value: offset + i * rowHeight });
		Object.defineProperty(li, 'offsetHeight', { value: rowHeight - 20 });
		Object.defineProperty(li, 'offsetParent', { value: null, configurable: true });
		list.appendChild(li);
	});
	const other = document.createElement('li');
	list.appendChild(other);
	document.body.appendChild(root);
	return root;
}

afterEach(() => {
	document.body.innerHTML = '';
});

describe('cardPositions', () => {
	it('places each card in the viewport scrolled to `scrollY`, in order', () => {
		const root = grid(['a', 'b', 'c']);
		expect(cardPositions(root, 350)).toEqual([
			{ id: 'a', top: -250, bottom: 30 },
			{ id: 'b', top: 50, bottom: 330 },
			{ id: 'c', top: 350, bottom: 630 }
		]);
	});

	it('adds up the offsets of the positioned boxes a card sits in', () => {
		const root = grid(['a']);
		const box = document.createElement('section');
		Object.defineProperty(box, 'offsetTop', { value: 40 });
		Object.defineProperty(box, 'offsetParent', { value: null });
		const card = root.querySelector('li')!;
		Object.defineProperty(card, 'offsetParent', { value: box, configurable: true });
		expect(cardPositions(root, 0)).toEqual([{ id: 'a', top: 140, bottom: 420 }]);
	});

	it('finds no cards without a page', () => {
		expect(cardPositions(null, 0)).toEqual([]);
	});
});

describe('scrollToAnchor', () => {
	it('scrolls so the card sits where it sat when the reader left', () => {
		const root = grid(['a', 'b', 'c']);
		const win = { scrollTo: vi.fn() };
		scrollToAnchor(root, { id: 'c', top: -40 }, win);
		expect(win.scrollTo).toHaveBeenCalledWith({ top: 740, left: 0, behavior: 'instant' });
	});

	it('opens at the top when the card is not on the page, or there is none', () => {
		const root = grid(['a']);
		const win = { scrollTo: vi.fn() };
		scrollToAnchor(root, { id: 'gone', top: 0 }, win);
		scrollToAnchor(root, null, win);
		scrollToAnchor(null, { id: 'a', top: 0 }, win);
		expect(win.scrollTo.mock.calls).toEqual([
			[{ top: 0, left: 0, behavior: 'instant' }],
			[{ top: 0, left: 0, behavior: 'instant' }],
			[{ top: 0, left: 0, behavior: 'instant' }]
		]);
	});

	it('never scrolls above the top of the page', () => {
		const root = grid(['a']);
		const win = { scrollTo: vi.fn() };
		scrollToAnchor(root, { id: 'a', top: 400 }, win);
		expect(win.scrollTo).toHaveBeenCalledWith({ top: 0, left: 0, behavior: 'instant' });
	});
});
