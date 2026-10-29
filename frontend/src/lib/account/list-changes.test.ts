import { describe, expect, it, vi } from 'vitest';

import {
	changeOf,
	createListChanges,
	entryNeedsReread,
	railNeedsReread,
	writeEnd,
	type ListChange
} from './list-changes';
import { SupersededError } from './save-when-allowed';

describe('writeEnd', () => {
	it('reads a write the tracker took as landed', () => {
		expect(writeEnd({ status: 'fulfilled', value: 'saved' })).toBe('landed');
	});

	it('reads a write dropped unsent — superseded, or its account gone — as dropped', () => {
		expect(writeEnd({ status: 'rejected', reason: new SupersededError() })).toBe('dropped');
	});

	it('reads any other refusal or failure as failed', () => {
		expect(writeEnd({ status: 'rejected', reason: new Error('offline') })).toBe('failed');
		expect(writeEnd({ status: 'rejected', reason: 'odd' })).toBe('failed');
	});
});

describe('changeOf', () => {
	it('names the trackers whose write landed or failed, in tracker order', () => {
		expect(changeOf('12', 'editor', { mal: 'failed', anilist: 'landed' })).toEqual({
			kitsuId: '12',
			providers: ['anilist', 'mal'],
			landed: ['anilist'],
			source: 'editor'
		});
	});

	it('leaves out a tracker whose write was dropped: nothing of it was sent', () => {
		expect(changeOf('12', 'progress', { anilist: 'dropped', mal: 'landed' })).toEqual({
			kitsuId: '12',
			providers: ['mal'],
			landed: ['mal'],
			source: 'progress'
		});
	});

	it('is no change when every write was dropped, or there was none', () => {
		expect(changeOf('12', 'editor', { anilist: 'dropped', mal: 'dropped' })).toBeNull();
		expect(changeOf('12', 'editor', {})).toBeNull();
	});
});

const change = (
	providers: ListChange['providers'],
	source: ListChange['source'] = 'editor'
): ListChange => ({
	kitsuId: '12',
	providers,
	landed: providers,
	source
});

describe('railNeedsReread', () => {
	it.each(['anilist', 'mal'] as const)(
		'rereads %s after failed progress because earlier mapped segments can already be cached',
		(provider) => {
			expect(
				railNeedsReread({ kitsuId: '99', providers: [provider], landed: [], source: 'progress' }, [
					provider
				])
			).toBe(true);
		}
	);
	it('does not reread for failed progress on an unshown tracker or for an empty change', () => {
		expect(
			railNeedsReread({ kitsuId: '99', providers: ['mal'], landed: [], source: 'progress' }, [
				'anilist'
			])
		).toBe(false);
		expect(
			railNeedsReread({ kitsuId: '99', providers: [], landed: [], source: 'progress' }, ['anilist'])
		).toBe(false);
	});

	it('is wanted when the change touched a tracker the row shows', () => {
		expect(railNeedsReread(change(['anilist']), ['anilist', 'mal'])).toBe(true);
		expect(railNeedsReread(change(['mal'], 'progress'), ['mal'])).toBe(true);
	});

	it('is not wanted when every editor write the row would show failed', () => {
		expect(
			railNeedsReread({ kitsuId: '12', providers: ['anilist'], landed: [], source: 'editor' }, [
				'anilist'
			])
		).toBe(false);
		expect(
			railNeedsReread(
				{ kitsuId: '12', providers: ['anilist', 'mal'], landed: ['mal'], source: 'editor' },
				['anilist']
			)
		).toBe(false);
	});

	it('is not wanted for a tracker the row does not show', () => {
		expect(railNeedsReread(change(['mal']), ['anilist'])).toBe(false);
		expect(railNeedsReread(change(['anilist']), [])).toBe(false);
	});
});

describe('entryNeedsReread', () => {
	it("is wanted on the show's own page for a progress write", () => {
		expect(entryNeedsReread(change(['anilist'], 'progress'), '12')).toBe(true);
	});

	it("is left to the editor for the editor's own save, which reconciles itself", () => {
		expect(entryNeedsReread(change(['anilist'], 'editor'), '12')).toBe(false);
	});

	it('is not wanted on another show, or on no show', () => {
		expect(entryNeedsReread(change(['anilist'], 'progress'), '13')).toBe(false);
		expect(entryNeedsReread(change(['anilist'], 'progress'), null)).toBe(false);
	});
});

describe('createListChanges', () => {
	it('tells every listener each change, until it stops listening', () => {
		const channel = createListChanges();
		const a = vi.fn();
		const b = vi.fn();
		const stopA = channel.subscribe(a);
		channel.subscribe(b);
		const first = change(['anilist']);
		channel.announce(first);
		stopA();
		const second = change(['mal']);
		channel.announce(second);
		expect(a.mock.calls).toEqual([[first]]);
		expect(b.mock.calls).toEqual([[first], [second]]);
	});

	it('keeps telling the others when one listener throws', () => {
		const channel = createListChanges();
		const after = vi.fn();
		channel.subscribe(() => {
			throw new Error('a view that went wrong');
		});
		channel.subscribe(after);
		channel.announce(change(['anilist']));
		expect(after).toHaveBeenCalledOnce();
	});

	it('lets a listener stop listening while it is being told', () => {
		const channel = createListChanges();
		const later = vi.fn();
		const stop = channel.subscribe(() => stop());
		channel.subscribe(later);
		channel.announce(change(['anilist']));
		channel.announce(change(['anilist']));
		expect(later).toHaveBeenCalledTimes(2);
	});
});
