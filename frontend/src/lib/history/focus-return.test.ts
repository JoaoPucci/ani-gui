// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from 'vitest';
import { focusBack, openerOf } from './focus-return';

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
