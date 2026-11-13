import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { IDLE, arrival, release, type Moment } from './arrival';
import { markRead, raise, resolveKey, toneOf, type Notice, type NoticeKind } from './model';

const NOW = 1_800_000_000_000;
const row = (key: string, kind: NoticeKind): Notice =>
	raise([], { key, kind }, NOW).notice as Notice;
const PLAYING: Moment = { playing: true, fullscreen: false };
const FULL: Moment = { playing: false, fullscreen: true };
const FULL_PLAYING: Moment = { playing: true, fullscreen: true };

describe('how a notice arrives', () => {
	it('toasts news while nothing plays', () => {
		for (const kind of ['outage', 'download-done', 'download-failed'] as const) {
			expect(arrival(row('k', kind), true, IDLE)).toBe('toast');
		}
	});

	it('never toasts the update: its badge announces it', () => {
		expect(arrival(row('update', 'update'), true, IDLE)).toBe('quiet');
	});

	it('says nothing for a change that is not news', () => {
		expect(arrival(row('k', 'download-failed'), false, IDLE)).toBe('quiet');
	});

	it('lets only failures toast while a video plays', () => {
		expect(arrival(row('k', 'download-failed'), true, PLAYING)).toBe('toast');
		expect(arrival(row('k', 'outage'), true, PLAYING)).toBe('quiet');
		expect(arrival(row('k', 'download-done'), true, PLAYING)).toBe('quiet');
		const resolved = resolveKey([row('o', 'outage')], 'o', NOW).notice as Notice;
		expect(arrival(resolved, true, PLAYING)).toBe('quiet');
	});

	it('holds a toast that would show over fullscreen', () => {
		expect(arrival(row('k', 'download-failed'), true, FULL_PLAYING)).toBe('hold');
		expect(arrival(row('k', 'download-done'), true, FULL)).toBe('hold');
		expect(arrival(row('k', 'download-done'), true, FULL_PLAYING)).toBe('quiet');
	});
});

describe('releasing held toasts', () => {
	const failed = row('f', 'download-failed');
	const done = row('d', 'download-done');

	it('keeps holding while still fullscreen', () => {
		expect(release(['f'], [failed], FULL)).toEqual({ show: [], held: ['f'] });
	});

	it('shows each held row once on leaving fullscreen', () => {
		expect(release(['f', 'f', 'd'], [failed, done], IDLE)).toEqual({
			show: [failed, done],
			held: []
		});
	});

	it('lets go of a row read, dismissed, or no longer allowed to toast meanwhile', () => {
		expect(release(['f'], markRead([failed], 'f'), IDLE).show).toEqual([]);
		expect(release(['gone'], [failed], IDLE).show).toEqual([]);
		expect(release(['d', 'f'], [done, failed], PLAYING).show).toEqual([failed]);
	});

	it('never shows anything while fullscreen, whatever is held', () => {
		const kinds: NoticeKind[] = ['update', 'outage', 'download-done', 'download-failed'];
		fc.assert(
			fc.property(
				fc.array(fc.constantFrom(...kinds), { maxLength: 8 }),
				fc.boolean(),
				(ks, playing) => {
					const list = ks.map((k, i) => row(`k${i}`, k));
					const at: Moment = { playing, fullscreen: true };
					expect(
						release(
							list.map((n) => n.key),
							list,
							at
						).show
					).toEqual([]);
					for (const n of list) expect(arrival(n, true, at)).not.toBe('toast');
					const out = release(
						list.map((n) => n.key),
						list,
						{ playing, fullscreen: false }
					);
					for (const n of out.show) if (playing) expect(toneOf(n)).toBe('failure');
				}
			)
		);
	});
});
