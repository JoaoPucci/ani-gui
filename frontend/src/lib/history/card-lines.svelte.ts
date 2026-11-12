/**
 * The history line each Continue card stands for, and whether the card
 * can correct it, as the backend answers them (`commands/card_line.rs`).
 *
 * A guessed card's play can land on another listing and write a row of
 * its own there; while the line it came from is still in the history
 * and shows as the same show, the card stands for that line. The
 * backend decides it from what only Home holds: the rows as Continue
 * Watching matches them, each with its show and whether that is a
 * guess.
 *
 * Home hands over the rows each time they change. The cards draw the
 * latest answer: until the first arrives, each card stands for its own
 * row and offers no correction, as before its match was read, and while
 * an answer for changed rows is out they keep the last one, so a pill
 * does not blink out from under a pointer or keyboard focus whenever
 * another card's match lands. A click waits for the answer for the rows
 * as they are (`settled`) and acts on it (`clickLineOf`); when that
 * answer failed, it acts on each row itself, as before the cards had
 * one. One question is out at a time; rows that change meanwhile are
 * asked once it answers. A failed answer leaves the last one drawn,
 * until the rows change again.
 */

import type { CardLine, CardLineRow } from '$lib/api';

export class CardLines {
	#ask: (rows: CardLineRow[]) => Promise<Record<string, CardLine>>;
	#answer = $state<{ key: string; lines: Record<string, CardLine> } | null>(null);
	#wanted = $state('');
	#rows: CardLineRow[] = [];
	#out: Promise<void> | null = null;
	#waiters: (() => void)[] = [];

	constructor(ask: (rows: CardLineRow[]) => Promise<Record<string, CardLine>>) {
		this.#ask = ask;
	}

	/** The rows as Continue Watching shows them now. */
	update(rows: CardLineRow[]): void {
		const key = JSON.stringify(rows);
		if (key === this.#wanted) return;
		this.#wanted = key;
		this.#rows = rows;
		this.#out ??= this.#send();
	}

	async #send(): Promise<void> {
		while (this.#answer?.key !== this.#wanted) {
			const key = this.#wanted;
			const lines = await this.#ask(this.#rows).catch(() => null);
			if (lines === null) break;
			this.#answer = { key, lines };
		}
		this.#out = null;
		for (const done of this.#waiters.splice(0)) done();
	}

	#current(id: string): CardLine | undefined {
		return this.#answer?.lines[id];
	}

	/** The line the card showing the row `id` stands for. */
	lineOf(id: string): string {
		return this.#current(id)?.line ?? id;
	}

	/** The line a click on the card showing the row `id` acts on: the
	 *  answer's for the rows as they are, else the row itself. Read once
	 *  `settled`. */
	clickLineOf(id: string): string {
		return (this.#answer?.key === this.#wanted ? this.#answer.lines[id]?.line : undefined) ?? id;
	}

	/** Whether the card showing the row `id` can correct its line. */
	correctable(id: string): boolean {
		return this.#current(id)?.correctable === true;
	}

	/** Settles once no question is out: `clickLineOf` then answers for
	 *  the rows as they are, or, after a failed answer, with each row. */
	settled(): Promise<void> {
		if (!this.#out) return Promise.resolve();
		return new Promise((resolve) => this.#waiters.push(resolve));
	}
}
