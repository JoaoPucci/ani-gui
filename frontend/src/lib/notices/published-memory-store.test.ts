import { describe, expect, it, vi } from 'vitest';
import { createPublishedMemoryStore } from './published-memory-store';
import { PUBLISHED_STORAGE_KEY, serializeMemory } from './published-memory';
import { raise, type Notice, type RaiseInput } from './model';

const make = (i: RaiseInput): Notice => raise([], i, 1).notice as Notice;
const remote = (id: string, read = false): Notice => ({
	...make({ key: `remote:${id}`, kind: 'remote', data: { title: id } }),
	read
});

function fakeStorage(initial: Record<string, string> = {}) {
	const items = new Map(Object.entries(initial));
	return {
		items,
		getItem: vi.fn((k: string) => items.get(k) ?? null),
		setItem: vi.fn((k: string, v: string) => void items.set(k, v))
	};
}

describe('the stored memory of published notices', () => {
	it('starts from what an earlier launch stored', () => {
		const storage = fakeStorage({
			[PUBLISHED_STORAGE_KEY]: serializeMemory({ read: ['a'], dismissed: ['b'] })
		});
		expect(createPublishedMemoryStore(storage).current).toEqual({ read: ['a'], dismissed: ['b'] });
	});

	it('starts empty when storage is missing or refuses to be read', () => {
		expect(createPublishedMemoryStore(null).current).toEqual({ read: [], dismissed: [] });
		const refusing = {
			getItem: () => {
				throw new Error('denied');
			},
			setItem: vi.fn()
		};
		expect(createPublishedMemoryStore(refusing).current).toEqual({ read: [], dismissed: [] });
	});

	it("writes what the center's list changes, read and dismissed", () => {
		const storage = fakeStorage();
		const store = createPublishedMemoryStore(storage);
		store.observe([remote('a'), remote('b')], [remote('a', true)]);
		expect(store.current).toEqual({ read: ['a'], dismissed: ['b'] });
		expect(storage.items.get(PUBLISHED_STORAGE_KEY)).toBe(
			serializeMemory({ read: ['a'], dismissed: ['b'] })
		);
	});

	it('does not write when nothing it remembers changed', () => {
		const storage = fakeStorage();
		const store = createPublishedMemoryStore(storage);
		store.observe([remote('a')], [remote('a')]);
		store.keepPublished(['x']);
		expect(storage.setItem).not.toHaveBeenCalled();
		store.observe([remote('a')], [remote('a', true)]);
		store.observe([remote('a', true)], [remote('a', true)]);
		expect(storage.setItem).toHaveBeenCalledTimes(1);
	});

	it('keeps what it learns for the session when storage refuses the write', () => {
		const refusing = {
			getItem: () => null,
			setItem: vi.fn(() => {
				throw new Error('quota');
			})
		};
		const store = createPublishedMemoryStore(refusing);
		store.observe([remote('a')], []);
		expect(store.current).toEqual({ read: [], dismissed: ['a'] });
		expect(refusing.setItem).toHaveBeenCalledTimes(1);
	});

	it('moves the ids still published to the end of each list, and stores that', () => {
		const storage = fakeStorage({
			[PUBLISHED_STORAGE_KEY]: serializeMemory({ read: ['a', 'b'], dismissed: [] })
		});
		const store = createPublishedMemoryStore(storage);
		expect(store.keepPublished(['a'])).toEqual({ read: ['b', 'a'], dismissed: [] });
		expect(store.current).toEqual({ read: ['b', 'a'], dismissed: [] });
		expect(storage.items.get(PUBLISHED_STORAGE_KEY)).toBe(
			serializeMemory({ read: ['b', 'a'], dismissed: [] })
		);
	});
});
