import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import type { HistoryEntry } from '$lib/api';
import { cardRow } from './card-row';

const CARD = 'hianime:jojos-bizarre-adventure-2000-5182';
const PLAYED = 'hianime:jojos-bizarre-adventure-2012-708';
const SERIES_2012 = '7158';
const OVA_2000 = '4013';

const row = (id: string, guessedFrom?: string): HistoryEntry =>
	guessedFrom
		? { id, ep_no: '2', title: "JoJo's Bizarre Adventure", guessed_from: guessedFrom }
		: { id, ep_no: '1', title: "JoJo's Bizarre Adventure" };

describe('cardRow — the line a Continue card stands for', () => {
	it('stands the JoJo card for the 2000 line its guessed play came from, not the 2012 row it wrote', () => {
		const rows = [row(PLAYED, CARD), row(CARD)];
		const matches = { [PLAYED]: { id: SERIES_2012 }, [CARD]: { id: SERIES_2012 } };
		expect(cardRow(rows[0], rows, matches)).toBe(CARD);
	});

	it('once the 2000 line is linked to its show, the 2012 row is its own card again', () => {
		const rows = [row(PLAYED, CARD), row(CARD)];
		const matches = { [PLAYED]: { id: SERIES_2012 }, [CARD]: { id: OVA_2000 } };
		expect(cardRow(rows[0], rows, matches)).toBe(PLAYED);
		expect(cardRow(rows[1], rows, matches)).toBe(CARD);
	});

	it('is the row itself when the line it came from is gone, or not matched yet', () => {
		const played = row(PLAYED, CARD);
		expect(cardRow(played, [played], { [PLAYED]: { id: SERIES_2012 } })).toBe(PLAYED);
		expect(
			cardRow(played, [played, row(CARD)], { [PLAYED]: { id: SERIES_2012 }, [CARD]: undefined })
		).toBe(PLAYED);
		expect(
			cardRow(played, [played, row(CARD)], { [PLAYED]: { id: SERIES_2012 }, [CARD]: null })
		).toBe(PLAYED);
	});

	it('is the row itself for a row no guessed play wrote', () => {
		const card = row(CARD);
		expect(cardRow(card, [card], { [CARD]: { id: SERIES_2012 } })).toBe(CARD);
	});
});

describe('cardRow — properties', () => {
	const ids = fc.constantFrom('a', 'b', 'c', 'd');
	const kitsu = fc.option(fc.constantFrom('k1', 'k2'), { nil: undefined });
	const world = fc
		.uniqueArray(fc.record({ id: ids, from: fc.option(ids, { nil: undefined }) }), {
			minLength: 1,
			selector: (r) => r.id
		})
		.chain((rs) =>
			fc.tuple(
				fc.constant(rs.map((r) => (r.from ? row(r.id, r.from) : row(r.id)))),
				fc.tuple(...rs.map(() => kitsu)),
				fc.nat({ max: rs.length - 1 })
			)
		);

	it('names the shown row, or the row it came from, and only one shown as the same show', () => {
		fc.assert(
			fc.property(world, ([rows, ks, at]) => {
				const matches = Object.fromEntries(
					rows.map((r, i) => [r.id, ks[i] === undefined ? undefined : { id: ks[i]! }])
				);
				const shown = rows[at];
				const stands = cardRow(shown, rows, matches);
				if (stands === shown.id) return;
				// Only the row a guessed play came from, still in the history,
				// matched as the same show the shown row is.
				expect(stands).toBe(shown.guessed_from);
				expect(rows.some((r) => r.id === stands)).toBe(true);
				expect(matches[stands]?.id).toBeDefined();
				expect(matches[stands]?.id).toBe(matches[shown.id]?.id);
			})
		);
	});

	it('stands for the row it came from whenever that row shows as the same show', () => {
		fc.assert(
			fc.property(world, ([rows, ks, at]) => {
				const matches = Object.fromEntries(
					rows.map((r, i) => [r.id, ks[i] === undefined ? undefined : { id: ks[i]! }])
				);
				const shown = rows[at];
				const from = shown.guessed_from;
				const sameShow =
					from !== undefined &&
					from !== shown.id &&
					rows.some((r) => r.id === from) &&
					matches[shown.id] !== undefined &&
					matches[from]?.id === matches[shown.id]?.id;
				expect(cardRow(shown, rows, matches) !== shown.id).toBe(sameShow);
			})
		);
	});
});
