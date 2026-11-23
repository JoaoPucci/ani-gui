// The guessed Continue card a show page was opened from, carried in
// the URL the player's links to the page build.

import { describe, expect, it } from 'vitest';
import * as fc from 'fast-check';
import { guessedQuery, guessedRowOf } from './guessed-context';

describe('guessedQuery', () => {
	it('names the card row of a guessed session', () => {
		expect(guessedQuery(true, 'hianime:black-clover-949')).toBe(
			'?guessed=hianime%3Ablack-clover-949'
		);
	});

	it('carries nothing for a session no guess opened, or with no row', () => {
		expect(guessedQuery(false, 'hianime:black-clover-949')).toBe('');
		expect(guessedQuery(true, null)).toBe('');
		expect(guessedQuery(true, '')).toBe('');
	});

	it('reads back the row it wrote, whatever the row', () => {
		fc.assert(
			fc.property(fc.string({ minLength: 1, maxLength: 40 }), (row) => {
				const query = guessedQuery(true, row);
				expect(guessedRowOf(new URLSearchParams(query))).toBe(row);
			})
		);
	});
});

describe('guessedRowOf', () => {
	it('is null for a page opened any other way', () => {
		expect(guessedRowOf(new URLSearchParams(''))).toBeNull();
		expect(guessedRowOf(new URLSearchParams('?page=2'))).toBeNull();
		expect(guessedRowOf(new URLSearchParams('?guessed='))).toBeNull();
	});
});
