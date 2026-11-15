/**
 * The history line each Continue card stands for, and whether the card
 * can correct it, as the backend answers them (`commands/card_line.rs`).
 *
 * A card's play can land on another listing and write a row of its own
 * there; while the line it came from is still in the history and shows
 * as the same show — as a guess, or named for it — the card stands for
 * that line, and so does the card of a row whose own listing holds a
 * line named for the same show. The backend decides it from what only
 * Home holds: the rows as Continue Watching matches them, each with its
 * show, whether that is a guess and whether the user named it.
 *
 * Home hands over the rows each time they change. The cards draw the
 * latest answer: until the first arrives, each card stands for its own
 * row and offers no correction, as before its match was read, and while
 * an answer for changed rows is out they keep the last one, so a pill
 * does not blink out from under a pointer or keyboard focus whenever
 * another card's match lands. A click waits for the answer for the rows
 * as they are (`settled`), asking for it again when the last one failed,
 * and acts on it (`clickLineOf`). With none, a click has no line: the
 * card may be drawn from an answer for other rows, standing for another
 * line than its own row, so acting on the row would correct a line the
 * card does not show. A pill's click names the line the card was drawn
 * with, and has no line when the answer it waited for names another. One question is out at a time; rows that change
 * meanwhile are asked once it answers. A failed answer leaves the last
 * one drawn.
 */

import type { CardLine, CardLineRow } from '$lib/api';

export class CardLines {
	#ask: (rows: CardLineRow[]) => Promise<Record<string, CardLine>>;
	#answer = $state<{ key: string; lines: Record<string, CardLine> } | null>(null);
	#wanted = $state('');
	#rows: CardLineRow[] = [];
	#out: Promise<void> | null = null;
	#failed = false;
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
		this.#start();
	}

	// A send starts only with something to ask: one that asks nothing
	// would finish before it was stored, and stay stored for good.
	#start(): void {
		if (!this.#out && this.#answer?.key !== this.#wanted) this.#out = this.#send();
	}

	async #send(): Promise<void> {
		while (this.#answer?.key !== this.#wanted) {
			const key = this.#wanted;
			const lines = await this.#ask(this.#rows).catch(() => null);
			this.#failed = lines === null;
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
	 *  answer's for the rows as they are, or `null` without one, and
	 *  `null` when it is not `drawn`, the line the card was drawn with
	 *  when it was clicked. Read once `settled`. */
	clickLineOf(id: string, drawn?: string): string | null {
		if (this.#answer?.key !== this.#wanted) return null;
		const line = this.#answer.lines[id]?.line ?? id;
		return drawn === undefined || line === drawn ? line : null;
	}

	/** Whether the card showing the row `id` can correct its line. */
	correctable(id: string): boolean {
		return this.#current(id)?.correctable === true;
	}

	/** Settles once no question is out, asking once more when the last
	 *  answer failed: `clickLineOf` then answers for the rows as they
	 *  are, or, when that failed too, with no line. */
	settled(): Promise<void> {
		if (this.#failed) this.#start();
		if (!this.#out) return Promise.resolve();
		return new Promise((resolve) => this.#waiters.push(resolve));
	}
}
