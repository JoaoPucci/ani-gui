// What the switch step says about the picked show before the switch:
// where the backend opens it, read once per pick, latest pick wins.

import { describe, expect, it, vi } from 'vitest';
import { SwitchPreview } from './switch-preview.svelte';

function deferred<T>() {
	let resolve!: (v: T) => void;
	let reject!: (e: unknown) => void;
	const promise = new Promise<T>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
}

describe('SwitchPreview', () => {
	it('is pending while the pick is read, then names the episode the backend opens', async () => {
		const answer = deferred<number | null>();
		const p = new SwitchPreview(() => answer.promise);
		p.ask('4013');
		expect(p.ownFor('4013')).toBe('pending');
		answer.resolve(7);
		await answer.promise;
		await Promise.resolve();
		expect(p.ownFor('4013')).toEqual({ episode: '7' });
	});

	it("says the card's episode stands when the pick is not further along", async () => {
		const p = new SwitchPreview(async () => null);
		p.ask('4013');
		await vi.waitFor(() => expect(p.ownFor('4013')).toBeNull());
	});

	it('claims nothing when the read fails: the switch itself asks again', async () => {
		// "It picks up episode 2 at 1:14" would be a claim the switch may
		// not keep: it reads again and may open the pick's own episode.
		const fail = vi.fn(async () => Promise.reject(new Error('down')));
		const p = new SwitchPreview(fail);
		p.ask('4013');
		await vi.waitFor(() => expect(fail).toHaveBeenCalled());
		await Promise.resolve();
		await Promise.resolve();
		expect(p.ownFor('4013')).toBe('pending');
	});

	it('answers only for the pick it was asked about, the latest one', async () => {
		const first = deferred<number | null>();
		const ask = vi.fn((id: string) => (id === 'a' ? first.promise : Promise.resolve(9)));
		const p = new SwitchPreview(ask);
		p.ask('a');
		p.ask('b');
		await vi.waitFor(() => expect(p.ownFor('b')).toEqual({ episode: '9' }));
		first.resolve(7);
		await first.promise;
		await Promise.resolve();
		expect(p.ownFor('a')).toBe('pending');
		expect(p.ownFor('b')).toEqual({ episode: '9' });
	});

	it('asks once for the same pick', () => {
		const ask = vi.fn(async () => null);
		const p = new SwitchPreview(ask);
		p.ask('a');
		p.ask('a');
		expect(ask).toHaveBeenCalledTimes(1);
	});
});
