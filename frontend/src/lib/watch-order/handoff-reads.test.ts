import { describe, expect, it, vi } from 'vitest';
import { createFranchiseReads, createNamedReads } from './handoff-reads';
import type { FranchiseData } from './types';

const franchise = (anchor: string): FranchiseData => ({
	anchor,
	entries: [],
	links: [],
	versions: [],
	films: [],
	sides: [],
	recaps: [],
	parents: {},
	complete: true
});

const flush = () => new Promise((r) => setTimeout(r, 0));

describe('createFranchiseReads', () => {
	it('reads a series once however often it is wanted, and hands it on', async () => {
		const read = vi.fn(async (id: string) => franchise(id));
		const settled = vi.fn();
		const reads = createFranchiseReads(read, settled);
		reads.want('7');
		reads.want('7');
		await flush();
		reads.want('7');
		await flush();
		expect(read).toHaveBeenCalledTimes(1);
		expect(settled).toHaveBeenCalledTimes(1);
		expect(settled).toHaveBeenCalledWith('7', franchise('7'));
	});

	it('reads each series it is asked for', async () => {
		const read = vi.fn(async (id: string) => franchise(id));
		const settled = vi.fn();
		const reads = createFranchiseReads(read, settled);
		reads.want('7');
		reads.want('8');
		await flush();
		expect(read.mock.calls.map(([id]) => id)).toEqual(['7', '8']);
		expect(settled).toHaveBeenCalledWith('8', franchise('8'));
	});

	it('settles a failed read as nothing, without asking again', async () => {
		const read = vi.fn(async () => {
			throw new Error('503');
		});
		const settled = vi.fn();
		const reads = createFranchiseReads(read, settled);
		reads.want('7');
		await flush();
		reads.want('7');
		await flush();
		expect(read).toHaveBeenCalledTimes(1);
		expect(settled).toHaveBeenCalledWith('7', null);
	});
});

describe('createNamedReads', () => {
	it('asks the resume lookup about an id once, and says whether a row resumes it', async () => {
		const read = vi.fn(async (ids: readonly string[]) =>
			Object.fromEntries(ids.map((id) => [id, id === '20' ? { ep_no: '3' } : null]))
		);
		const settled = vi.fn();
		const reads = createNamedReads(read, settled);
		reads.want('20');
		reads.want('20');
		reads.want('30');
		await flush();
		reads.want('20');
		await flush();
		expect(read).toHaveBeenCalledTimes(2);
		expect(read).toHaveBeenCalledWith(['20']);
		expect(settled).toHaveBeenCalledWith('20', true);
		expect(settled).toHaveBeenCalledWith('30', false);
	});

	it('settles a lookup that fails as unknown', async () => {
		const settled = vi.fn();
		const reads = createNamedReads(() => Promise.reject(new Error('no backend')), settled);
		reads.want('20');
		await flush();
		expect(settled).toHaveBeenCalledWith('20', null);
	});
});
