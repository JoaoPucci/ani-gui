import { describe, expect, it } from 'vitest';
import { AccountApiError } from './api';
import { readEntryWithRetry, type ReadDeps } from './list-entry-read';
import type { EntryView } from './types';

/** Deps whose `readAll` answers from a script, one entry per attempt,
 *  and whose `sleep` records what it was asked and returns at once. */
function scripted(
	script: (() => Promise<(EntryView | null)[]>)[],
	opts: { isCurrent?: () => boolean } = {}
) {
	const sleeps: number[] = [];
	let attempt = 0;
	const deps: ReadDeps = {
		readAll: () => {
			const step = script[Math.min(attempt, script.length - 1)];
			attempt += 1;
			return step();
		},
		isCurrent: opts.isCurrent ?? (() => true),
		sleep: async (ms) => {
			sleeps.push(ms);
		}
	};
	return { deps, sleeps, attempts: () => attempt };
}

const fails = (e: unknown) => () => Promise.reject(e);
const answers = (views: (EntryView | null)[]) => () => Promise.resolve(views);

describe('readEntryWithRetry', () => {
	it('folds every tracker into the seed entry when all answer', async () => {
		const { deps, sleeps } = scripted([
			answers([
				{ status: 'watching', progress: 5 },
				{ status: 'planning', progress: 0 }
			])
		]);
		await expect(readEntryWithRetry(deps)).resolves.toEqual({
			kind: 'ok',
			entry: { status: 'watching', progress: 5 }
		});
		expect(sleeps).toEqual([]);
	});

	it('answers null when no tracker has the show', async () => {
		const { deps } = scripted([answers([null, null])]);
		await expect(readEntryWithRetry(deps)).resolves.toEqual({ kind: 'ok', entry: null });
	});

	it('waits the schedule between failures and takes the answer when one comes', async () => {
		const { deps, sleeps, attempts } = scripted([
			fails(new Error('no bearer for mal')),
			fails(new AccountApiError(502, '')),
			answers([{ status: 'completed', progress: 12 }])
		]);
		await expect(readEntryWithRetry(deps)).resolves.toEqual({
			kind: 'ok',
			entry: { status: 'completed', progress: 12 }
		});
		expect(sleeps).toEqual([1000, 2000]);
		expect(attempts()).toBe(3);
	});

	it('gives up after the schedule runs out, naming the last failure', async () => {
		const { deps, sleeps, attempts } = scripted([fails(new AccountApiError(503, ''))]);
		await expect(readEntryWithRetry(deps)).resolves.toEqual({
			kind: 'failed',
			last: { rateLimited: false, retryAfterSecs: null, envelope: null }
		});
		expect(sleeps).toEqual([1000, 2000, 4000, 8000]);
		expect(attempts()).toBe(5);
	});

	it('is superseded when the page moves on while a read is in flight', async () => {
		let current = true;
		const { deps } = scripted(
			[
				() => {
					current = false;
					return Promise.resolve([{ status: 'watching', progress: 1 }]);
				}
			],
			{ isCurrent: () => current }
		);
		await expect(readEntryWithRetry(deps)).resolves.toEqual({ kind: 'superseded' });
	});

	it('is superseded when the page moves on while a failed read is in flight', async () => {
		let current = true;
		const { deps, sleeps } = scripted(
			[
				() => {
					current = false;
					return Promise.reject(new Error('gone'));
				}
			],
			{ isCurrent: () => current }
		);
		await expect(readEntryWithRetry(deps)).resolves.toEqual({ kind: 'superseded' });
		expect(sleeps).toEqual([]);
	});

	it('is superseded when the page moves on during the wait', async () => {
		let current = true;
		const { deps, attempts } = scripted([fails(new Error('transient'))], {
			isCurrent: () => current
		});
		deps.sleep = async () => {
			current = false;
		};
		await expect(readEntryWithRetry(deps)).resolves.toEqual({ kind: 'superseded' });
		expect(attempts()).toBe(1);
	});
});
