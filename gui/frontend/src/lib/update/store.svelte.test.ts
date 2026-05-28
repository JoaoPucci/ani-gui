// @vitest-environment happy-dom
//
// Singleton runes-state store for the update notifier. happy-dom
// supplies the DOM globals Svelte's runtime asserts on.
import { beforeEach, describe, expect, it } from 'vitest';
import { updateStore } from './store.svelte';
import type { ReleaseInfo } from './release-parse';

const release: ReleaseInfo = {
	tag: 'v0.5.0',
	name: 'v0.5.0 — newer',
	url: 'https://github.com/JoaoPucci/ani-gui/releases/tag/v0.5.0',
	publishedAt: '2026-06-01T00:00:00Z',
	body: 'release notes'
};

describe('updateStore', () => {
	beforeEach(() => {
		// Singleton — reset to a clean slate between tests.
		updateStore.setAvailable(null);
		updateStore.closeDialog();
	});

	it('starts empty — no update, dialog closed', () => {
		expect(updateStore.available).toBeNull();
		expect(updateStore.hasUpdate).toBe(false);
		expect(updateStore.dialogOpen).toBe(false);
	});

	it('setAvailable(release) flips hasUpdate on', () => {
		updateStore.setAvailable(release);
		expect(updateStore.available).toEqual(release);
		expect(updateStore.hasUpdate).toBe(true);
	});

	it('setAvailable(null) clears the badge', () => {
		updateStore.setAvailable(release);
		updateStore.setAvailable(null);
		expect(updateStore.hasUpdate).toBe(false);
	});

	it('openDialog() opens the modal', () => {
		updateStore.setAvailable(release);
		updateStore.openDialog();
		expect(updateStore.dialogOpen).toBe(true);
	});

	it('closeDialog() closes the modal but leaves the badge alone', () => {
		updateStore.setAvailable(release);
		updateStore.openDialog();
		updateStore.closeDialog();
		expect(updateStore.dialogOpen).toBe(false);
		expect(updateStore.hasUpdate).toBe(true);
		// Re-opening from the badge is still possible.
		updateStore.openDialog();
		expect(updateStore.dialogOpen).toBe(true);
	});

	it('hasUpdate stays true after opening — pulse is ambient, not one-shot', () => {
		updateStore.setAvailable(release);
		updateStore.openDialog();
		expect(updateStore.hasUpdate).toBe(true);
		updateStore.closeDialog();
		expect(updateStore.hasUpdate).toBe(true);
	});
});
