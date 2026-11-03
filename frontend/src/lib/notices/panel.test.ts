import { describe, expect, it, vi } from 'vitest';
import { act, noticeToast, stepRow, viewOf } from './panel';
import { raise, type Notice, type RaiseInput } from './model';

const make = (i: RaiseInput): Notice => raise([], i, 1_800_000_000_000).notice as Notice;

describe('arrow keys between rows', () => {
	it('moves down and up and stops at the ends', () => {
		expect(stepRow('ArrowDown', 0, 3)).toBe(1);
		expect(stepRow('ArrowDown', 2, 3)).toBe(2);
		expect(stepRow('ArrowUp', 1, 3)).toBe(0);
		expect(stepRow('ArrowUp', 0, 3)).toBe(0);
	});

	it('enters the list from outside it at the first or last row', () => {
		expect(stepRow('ArrowDown', -1, 3)).toBe(0);
		expect(stepRow('ArrowUp', -1, 3)).toBe(2);
	});

	it('jumps with Home and End, and ignores other keys or an empty list', () => {
		expect(stepRow('Home', 2, 3)).toBe(0);
		expect(stepRow('End', 0, 3)).toBe(2);
		expect(stepRow('Enter', 0, 3)).toBeNull();
		expect(stepRow('ArrowDown', -1, 0)).toBeNull();
	});
});

describe('opening a notice', () => {
	it('opens its page, or in place while on the player', () => {
		expect(viewOf('outage:hianime', false)).toEqual({
			mode: 'page',
			path: '/notifications/outage%3Ahianime'
		});
		expect(viewOf('outage:hianime', true)).toEqual({ mode: 'in-place', key: 'outage:hianime' });
	});
});

describe('a notice as a toast', () => {
	it('carries its colour, words, a View action and a longer stay for problems', () => {
		const view = vi.fn();
		const failed = make({
			key: 'f',
			kind: 'download-failed',
			data: { title: 'Frieren', episode: '5' }
		});
		const t = noticeToast(failed, view);
		expect(t).toMatchObject({
			kind: 'error',
			message: 'A download failed · Frieren · Episode 5',
			duration: 8000,
			actionLabel: 'View'
		});
		t.onAction?.();
		expect(view).toHaveBeenCalledWith('f');
		expect(noticeToast(make({ key: 'd', kind: 'download-done' }), view).duration).toBe(5000);
		expect(noticeToast(make({ key: 'o', kind: 'outage' }), view).duration).toBe(8000);
	});
});

describe('a row action', () => {
	it('opens the update dialog, or reveals the download folder', () => {
		const deps = { openUpdate: vi.fn(), reveal: vi.fn() };
		act('open-update', make({ key: 'update', kind: 'update' }), deps);
		expect(deps.openUpdate).toHaveBeenCalled();
		act('reveal-folder', make({ key: 'd', kind: 'download-done', data: { destDir: '/v' } }), deps);
		expect(deps.reveal).toHaveBeenCalledWith('/v');
		act('reveal-folder', make({ key: 'd', kind: 'download-done' }), deps);
		expect(deps.reveal).toHaveBeenCalledTimes(1);
	});
});
