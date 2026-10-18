import { describe, expect, it } from 'vitest';
import { latestReads } from './latest-call';

/** A read whose answers the test gives, in the order it likes. */
function heldRead() {
	const answers: ((v: string) => void)[] = [];
	const read = () => new Promise<string | null>((r) => answers.push(r));
	return { read, answers };
}

describe('latestReads', () => {
	it('drops the answer to a read a later one for the same key has superseded', async () => {
		const { read, answers } = heldRead();
		const latest = latestReads(read);
		const older = latest('a');
		const newer = latest('a');
		answers[1]('new');
		answers[0]('old');
		expect(await newer).toBe('new');
		expect(await older).toBeNull();
	});

	it('keeps the latest answer whichever order the answers land in', async () => {
		const { read, answers } = heldRead();
		const latest = latestReads(read);
		const older = latest('a');
		const newer = latest('a');
		answers[0]('old');
		answers[1]('new');
		expect(await older).toBeNull();
		expect(await newer).toBe('new');
	});

	it('keeps each key to itself', async () => {
		const { read, answers } = heldRead();
		const latest = latestReads(read);
		const a = latest('a');
		const b = latest('b');
		answers[0]('a');
		answers[1]('b');
		expect(await a).toBe('a');
		expect(await b).toBe('b');
	});

	it('passes a failed read on as it failed', async () => {
		const latest = latestReads(async () => {
			throw new Error('offline');
		});
		await expect(latest('a')).rejects.toThrow('offline');
	});
});
