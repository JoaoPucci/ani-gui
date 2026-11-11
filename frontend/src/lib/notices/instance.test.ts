import { describe, expect, it } from 'vitest';
import { NoticeCenter } from './center.svelte';
import {
	browserStorage,
	noticeCenter,
	setMomentReader,
	setNoticeObserver,
	setNoticeToast
} from './instance';
import type { Notice } from './model';

describe('the window instance', () => {
	it('is a notice center whose toasts go where they are sent', () => {
		expect(noticeCenter).toBeInstanceOf(NoticeCenter);
		// Before the layout points it anywhere, a toast goes nowhere.
		noticeCenter.raise({ key: 'early', kind: 'download-failed' });
		noticeCenter.dismiss('early');
		const seen: Notice[] = [];
		setNoticeToast((n) => seen.push(n));
		noticeCenter.raise({ key: 'k', kind: 'download-failed' });
		expect(seen.map((n) => n.key)).toEqual(['k']);
		noticeCenter.dismiss('k');
	});

	it('reads storage from the window, or none where there is none or it throws', () => {
		const store = { getItem: () => null, setItem: () => {} };
		expect(browserStorage({ localStorage: store })).toBe(store);
		expect(browserStorage(undefined)).toBeNull();
		const throwing = {
			get localStorage(): Storage {
				throw new Error('denied');
			}
		};
		expect(browserStorage(throwing)).toBeNull();
	});
});

describe('what the window instance reads at an arrival', () => {
	it('asks the reader the layout set, and the last reported moment without one', () => {
		const seen: Notice[] = [];
		setNoticeToast((n) => seen.push(n));
		setMomentReader(() => ({ playing: true, fullscreen: false }));
		noticeCenter.raise({ key: 'quiet', kind: 'download-done' });
		expect(seen).toEqual([]);
		setMomentReader(null);
		noticeCenter.setMoment({ playing: false, fullscreen: false });
		noticeCenter.raise({ key: 'heard', kind: 'download-done' });
		expect(seen.map((n) => n.key)).toEqual(['heard']);
		noticeCenter.dismiss('quiet');
		noticeCenter.dismiss('heard');
	});
});

describe('who the window instance tells of a change', () => {
	it('tells the observer the layout set, and nobody without one', () => {
		const seen: Array<[number, number]> = [];
		setNoticeObserver((before, after) => seen.push([before.length, after.length]));
		noticeCenter.raise({ key: 'o', kind: 'download-failed' });
		noticeCenter.dismiss('o');
		expect(seen).toEqual([
			[0, 1],
			[1, 0]
		]);
		setNoticeObserver(null);
		noticeCenter.raise({ key: 'p', kind: 'download-failed' });
		noticeCenter.dismiss('p');
		expect(seen).toHaveLength(2);
	});
});
