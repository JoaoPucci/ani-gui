import { describe, expect, it } from 'vitest';
import { watchLaterRefreshSignal } from './watch-later-signal.svelte';

describe('watchLaterRefreshSignal', () => {
	it('bump() advances the version monotonically', () => {
		const start = watchLaterRefreshSignal.version;
		watchLaterRefreshSignal.bump();
		expect(watchLaterRefreshSignal.version).toBe(start + 1);
		watchLaterRefreshSignal.bump();
		expect(watchLaterRefreshSignal.version).toBe(start + 2);
	});
});
