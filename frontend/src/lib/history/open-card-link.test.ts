// Opening Home's picker for a guessed Continue card.

import { describe, expect, it } from 'vitest';
import {
	failedResumeGuess,
	openCardLink,
	type CardCheck,
	type OpenCardLinkDeps
} from './open-card-link';

function world(check: CardCheck | (() => Promise<CardCheck>), busy = () => false) {
	const log: string[] = [];
	const deps: OpenCardLinkDeps = {
		busy,
		dismissFailure: () => log.push('dismiss'),
		stillGuessed: async (row, matchId) => {
			log.push(`check:${row}:${matchId}`);
			return typeof check === 'function' ? check() : check;
		},
		checkFailed: () => log.push('check-failed'),
		start: (episode) => log.push(`start:${episode}`)
	};
	return { deps, log };
}

describe('openCardLink', () => {
	it('opens the picker on a card still guessed, naming the episode, the overlay gone first', async () => {
		const { deps, log } = world(true);
		expect(await openCardLink('row-1', '42', 3, deps)).toBe('opened');
		expect(log).toEqual(['dismiss', 'check:row-1:42', 'start:3']);
	});

	it('does nothing while a remove or Clear history is out', async () => {
		const { deps, log } = world(true, () => true);
		expect(await openCardLink('row-1', '42', 3, deps)).toBe('held');
		expect(log).toEqual([]);
	});

	it('says a check that could not be read, and opens nothing', async () => {
		const { deps, log } = world('unread');
		expect(await openCardLink('row-1', '42', 3, deps)).toBe('unread');
		expect(log).toEqual(['dismiss', 'check:row-1:42', 'check-failed']);
	});

	it('opens nothing, and says nothing, for a card settled since the cards were read', async () => {
		const { deps, log } = world(false);
		expect(await openCardLink('row-1', '42', 3, deps)).toBe('settled');
		expect(log).toEqual(['dismiss', 'check:row-1:42']);
	});

	it('opens nothing when a remove or clear began while the check was out', async () => {
		let busy = false;
		const { deps, log } = world(
			async () => {
				busy = true;
				return true;
			},
			() => busy
		);
		expect(await openCardLink('row-1', '42', 3, deps)).toBe('overtaken');
		expect(log).not.toContain('start:3');
	});
});

describe('failedResumeGuess', () => {
	const LINE = { id: 'hianime:black-clover-949', title: 'Black Clover' };
	const args = { guess: true, row: LINE.id, matchId: '12625', episode: 1 };
	const deps = (check: CardCheck, current = () => true) => ({
		line: (row: string) => (row === LINE.id ? LINE : undefined),
		stillGuessed: async () => check,
		isCurrent: current
	});

	it("offers the pick for a guess still standing, naming the line's title and the episode", async () => {
		expect(await failedResumeGuess(args, deps(true))).toEqual({
			row: LINE.id,
			matchId: '12625',
			title: 'Black Clover',
			episode: 1
		});
	});

	it('offers it for a check that could not be read, since the pick checks again', async () => {
		expect(await failedResumeGuess(args, deps('unread'))).toMatchObject({ row: LINE.id });
	});

	it('offers none for a card settled since, a play no guess opened, or a line gone', async () => {
		expect(await failedResumeGuess(args, deps(false))).toBeUndefined();
		expect(await failedResumeGuess({ ...args, guess: false }, deps(true))).toBeUndefined();
		expect(await failedResumeGuess({ ...args, row: 'gone' }, deps(true))).toBeUndefined();
	});

	it('shows nothing once the viewer moved on while the check was out', async () => {
		expect(
			await failedResumeGuess(
				args,
				deps(true, () => false)
			)
		).toBeNull();
	});
});
