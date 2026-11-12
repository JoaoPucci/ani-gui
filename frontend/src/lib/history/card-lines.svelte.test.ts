// Home's cards' lines, as the backend answers them: only the answer
// for the rows as they are now counts. Which line a card stands for is
// the backend's rule (commands/card_line.rs, its own tests).

import { describe, expect, it, vi } from 'vitest';
import type { CardLine, CardLineRow } from '$lib/api';
import { CardLines } from './card-lines.svelte';
import { cardLines } from '../../test/link-rules-fake';

const CARD = 'hianime:jojos-bizarre-adventure-2000-5182';
const PLAYED = 'hianime:jojos-bizarre-adventure-2012-708';

const rows = (cardShow: string): CardLineRow[] => [
	{ id: PLAYED, guessed_from: CARD, show: '7158', guess: true, named: false },
	{ id: CARD, guessed_from: null, show: cardShow, guess: true, named: false }
];

/** An ask the test answers by hand, in any order. */
function gated() {
	const asks: { rows: CardLineRow[]; answer: (ok: boolean) => void }[] = [];
	const ask = vi.fn(
		(r: CardLineRow[]) =>
			new Promise<Record<string, CardLine>>((resolve, reject) => {
				asks.push({
					rows: r,
					answer: (ok) => (ok ? resolve(cardLines(r)) : reject(new Error('down')))
				});
			})
	);
	return { ask, asks };
}

const tick = () => new Promise((r) => setTimeout(r, 0));

describe('CardLines', () => {
	it('stands each card for its own row, with no correction, until the backend answers', async () => {
		const { ask, asks } = gated();
		const lines = new CardLines(ask);
		lines.update(rows('7158'));
		expect(lines.lineOf(PLAYED)).toBe(PLAYED);
		expect(lines.correctable(PLAYED)).toBe(false);
		asks[0].answer(true);
		await lines.settled();
		expect(lines.lineOf(PLAYED)).toBe(CARD);
		expect(lines.correctable(PLAYED)).toBe(true);
	});

	it('asks once for rows that did not change', async () => {
		const { ask, asks } = gated();
		const lines = new CardLines(ask);
		lines.update(rows('7158'));
		asks[0].answer(true);
		await lines.settled();
		lines.update(rows('7158'));
		expect(ask).toHaveBeenCalledTimes(1);
	});

	it('keeps drawing the last answer while one for changed rows is out, and asks for the rows as they are', async () => {
		const { ask, asks } = gated();
		const lines = new CardLines(ask);
		lines.update(rows('7158'));
		asks[0].answer(true);
		await lines.settled();
		// Another row's match lands: the cards keep their lines and pills
		// until the answer for the rows as they are now.
		lines.update(rows('4013'));
		expect(lines.lineOf(PLAYED)).toBe(CARD);
		expect(lines.correctable(PLAYED)).toBe(true);
		expect(asks[1].rows).toEqual(rows('4013'));
		asks[1].answer(true);
		await lines.settled();
		expect(lines.lineOf(PLAYED)).toBe(PLAYED);
		expect(lines.lineOf(CARD)).toBe(CARD);
	});

	it('asks once for rows that changed while an answer was out, with the latest rows', async () => {
		const { ask, asks } = gated();
		const lines = new CardLines(ask);
		lines.update(rows('7158'));
		lines.update(rows('4013'));
		expect(ask).toHaveBeenCalledTimes(1);
		asks[0].answer(true);
		await tick();
		expect(asks[1].rows).toEqual(rows('4013'));
		asks[1].answer(true);
		await lines.settled();
		expect(lines.lineOf(PLAYED)).toBe(PLAYED);
	});

	it('settles once no ask is out, and at once when none is', async () => {
		const { ask, asks } = gated();
		const lines = new CardLines(ask);
		await lines.settled();
		lines.update(rows('7158'));
		let settled = false;
		void lines.settled().then(() => (settled = true));
		await tick();
		expect(settled).toBe(false);
		asks[0].answer(true);
		await tick();
		expect(settled).toBe(true);
	});

	it('stands each card for its own row after a failed first answer, and asks again when the rows change', async () => {
		const { ask, asks } = gated();
		const lines = new CardLines(ask);
		lines.update(rows('7158'));
		asks[0].answer(false);
		await lines.settled();
		expect(lines.lineOf(PLAYED)).toBe(PLAYED);
		expect(lines.correctable(PLAYED)).toBe(false);
		lines.update(rows('7159'));
		expect(ask).toHaveBeenCalledTimes(2);
	});
});
