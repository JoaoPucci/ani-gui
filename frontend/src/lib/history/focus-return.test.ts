// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from 'vitest';
import { cardOf, focusBack, openerOf } from './focus-return';

afterEach(() => {
	document.body.innerHTML = '';
});

describe('the opener a dialog remembers', () => {
	it('is what had focus, but never the page itself', () => {
		const button = document.createElement('button');
		document.body.append(button);
		expect(openerOf(button, document.body)).toBe(button);
		expect(openerOf(document.body, document.body)).toBeNull();
		expect(openerOf(null, document.body)).toBeNull();
	});
});

describe('where focus goes when the dialog closes', () => {
	it('is the opener while it is still in the document', () => {
		const opener = document.createElement('button');
		const card = document.createElement('button');
		document.body.append(opener, card);
		expect(focusBack(opener, () => card)).toBe(opener);
	});

	it("is the surface's fallback once the opener is gone, or when there was none", () => {
		const opener = document.createElement('button');
		const card = document.createElement('button');
		document.body.append(card);
		expect(focusBack(opener, () => card)).toBe(card);
		expect(focusBack(null, () => card)).toBe(card);
		expect(focusBack(opener)).toBeNull();
	});
});

describe('cardOf: the card focus goes back to on Home', () => {
	const cell = (line: string, row: string) => {
		const el = document.createElement('div');
		el.className = 'resume-cell';
		el.dataset.cardRow = line;
		el.dataset.entryId = row;
		const card = document.createElement('button');
		card.className = 'resume-card';
		el.appendChild(card);
		return { el, card };
	};

	it('is the card standing for the line', () => {
		const root = document.createElement('div');
		const jojo = cell('hianime:jojo-2000-5182', 'hianime:jojo-2012-708');
		root.appendChild(jojo.el);
		expect(cardOf(root, 'hianime:jojo-2000-5182')).toBe(jojo.card);
	});

	it('is the card showing the row, when the cards still draw another line for it', () => {
		// The cards' last answer named the 2000 line; the answer for the
		// rows as they are failed, so the click acted on the row itself.
		const root = document.createElement('div');
		const jojo = cell('hianime:jojo-2000-5182', 'hianime:jojo-2012-708');
		root.appendChild(jojo.el);
		expect(cardOf(root, 'hianime:jojo-2012-708')).toBe(jojo.card);
		expect(cardOf(root, 'hianime:other-1')).toBeNull();
	});
});
