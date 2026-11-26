// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { readScreen, sameRects, sameView, toRect } from './screen-read';

const box = (left: number, top: number, width: number, height: number) =>
	({ left, top, width, height, right: left + width, bottom: top + height }) as DOMRect;

describe('readScreen', () => {
	it('reads the window, the top bar, the place and the window buttons', () => {
		document.body.innerHTML =
			'<header class="topbar"><button class="winctl-btn"></button></header><div id="p"></div>';
		document.querySelector('.topbar')!.getBoundingClientRect = () => box(0, 0, 1000, 63.6);
		document.querySelector('.winctl-btn')!.getBoundingClientRect = () => box(950, 0, 40, 30);
		const place = document.getElementById('p')!;
		place.getBoundingClientRect = () => box(10, 100, 300, 50);
		const read = readScreen(document, place);
		expect(read.view).toEqual({ w: window.innerWidth, h: window.innerHeight, top: 64 });
		expect(read.rect).toEqual({ x: 10, y: 100, w: 300, h: 50 });
		expect(read.buttons).toEqual([{ x: 950, y: 0, w: 40, h: 30 }]);
	});

	it('reads no top bar as the window’s top, and no buttons as none', () => {
		document.body.innerHTML = '<div id="p"></div>';
		const read = readScreen(document, document.getElementById('p')!);
		expect(read.view.top).toBe(0);
		expect(read.buttons).toEqual([]);
	});
});

describe('comparisons', () => {
	it('compare views and rect lists', () => {
		expect(sameView({ w: 1, h: 2, top: 3 }, { w: 1, h: 2, top: 3 })).toBe(true);
		expect(sameView({ w: 1, h: 2, top: 3 }, { w: 1, h: 2, top: 4 })).toBe(false);
		const r = toRect(box(1, 2, 3, 4));
		expect(sameRects([r], [{ ...r }])).toBe(true);
		expect(sameRects([r], [])).toBe(false);
		expect(sameRects([r], [{ ...r, w: 9 }])).toBe(false);
	});
});
