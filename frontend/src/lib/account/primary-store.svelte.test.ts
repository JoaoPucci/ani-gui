import { describe, expect, it } from 'vitest';
import { primaryAccountStore } from './primary-store.svelte';

describe('primaryAccountStore', () => {
	it('holds null until set', () => {
		primaryAccountStore.set(null);
		expect(primaryAccountStore.value).toBeNull();
	});

	it('set() updates the held provider', () => {
		primaryAccountStore.set('mal');
		expect(primaryAccountStore.value).toBe('mal');
		primaryAccountStore.set('anilist');
		expect(primaryAccountStore.value).toBe('anilist');
		primaryAccountStore.set(null);
		expect(primaryAccountStore.value).toBeNull();
	});
});
