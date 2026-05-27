// @vitest-environment happy-dom
//
// Singleton runes-state store for the update notifier. happy-dom
// supplies the DOM globals Svelte's runtime asserts on plus a
// working `localStorage` so the dismissed-tag persistence path is
// exercised end-to-end.
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
		updateStore.dismissedTag = null;
		localStorage.clear();
	});

	it('starts empty — no update, no glow, dialog closed', () => {
		expect(updateStore.available).toBeNull();
		expect(updateStore.hasUpdate).toBe(false);
		expect(updateStore.glowing).toBe(false);
		expect(updateStore.dialogOpen).toBe(false);
	});

	it('setAvailable(release) flips hasUpdate + glowing on', () => {
		updateStore.setAvailable(release);
		expect(updateStore.available).toEqual(release);
		expect(updateStore.hasUpdate).toBe(true);
		expect(updateStore.glowing).toBe(true);
	});

	it('setAvailable(null) clears the badge', () => {
		updateStore.setAvailable(release);
		updateStore.setAvailable(null);
		expect(updateStore.hasUpdate).toBe(false);
		expect(updateStore.glowing).toBe(false);
	});

	it('openDialog() opens the modal and dismisses the current tag', () => {
		updateStore.setAvailable(release);
		updateStore.openDialog();
		expect(updateStore.dialogOpen).toBe(true);
		expect(updateStore.dismissedTag).toBe(release.tag);
		// Glow off once acknowledged for this tag — but badge still
		// shows (hasUpdate stays true).
		expect(updateStore.glowing).toBe(false);
		expect(updateStore.hasUpdate).toBe(true);
	});

	it('openDialog() persists the dismissed tag to localStorage', () => {
		updateStore.setAvailable(release);
		updateStore.openDialog();
		expect(localStorage.getItem('ani-gui-update-dismissed-tag')).toBe(release.tag);
	});

	it('closeDialog() leaves dismissedTag intact', () => {
		updateStore.setAvailable(release);
		updateStore.openDialog();
		updateStore.closeDialog();
		expect(updateStore.dialogOpen).toBe(false);
		expect(updateStore.dismissedTag).toBe(release.tag);
		// Re-opening is still possible from the badge.
		updateStore.openDialog();
		expect(updateStore.dialogOpen).toBe(true);
	});

	it('a newer tag re-lights the glow even after a previous dismiss', () => {
		updateStore.setAvailable(release);
		updateStore.openDialog();
		updateStore.closeDialog();
		expect(updateStore.glowing).toBe(false);

		const nextRelease: ReleaseInfo = { ...release, tag: 'v0.6.0' };
		updateStore.setAvailable(nextRelease);
		expect(updateStore.glowing).toBe(true);
	});

	it('openDialog() with no available release is a no-op for dismissedTag', () => {
		updateStore.setAvailable(null);
		updateStore.openDialog();
		expect(updateStore.dialogOpen).toBe(true);
		expect(updateStore.dismissedTag).toBeNull();
	});
});
