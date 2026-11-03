import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import {
	expectCaptions,
	extraBeside,
	noteCaptionChange,
	showingCaption,
	viewerPick,
	type CaptionState
} from './caption-watch';

const sub = (mode: string) => ({ kind: 'subtitles', mode });

describe('showingCaption', () => {
	it('is the subtitle track showing, or off', () => {
		expect(showingCaption([sub('disabled'), sub('showing')])).toBe(1);
		expect(showingCaption([sub('disabled'), sub('hidden')])).toBe('off');
		expect(showingCaption([{ kind: 'metadata', mode: 'showing' }, sub('disabled')])).toBe('off');
		expect(showingCaption([])).toBe('off');
	});
});

describe('viewerPick', () => {
	it('is no pick before the app has set the captions, or where they are as the app set them', () => {
		expect(viewerPick(null, 2)).toBeNull();
		expect(viewerPick(2, 2)).toBeNull();
		expect(viewerPick('off', 'off')).toBeNull();
	});

	it('is the change otherwise', () => {
		expect(viewerPick(2, 0)).toBe(0);
		expect(viewerPick(2, 'off')).toBe('off');
		expect(viewerPick('off', 1)).toBe(1);
	});

	it('is a pick exactly when the app had set the captions and they differ', () => {
		const state = fc.oneof(fc.constant<'off'>('off'), fc.nat(10));
		fc.assert(
			fc.property(fc.option(state, { nil: null }), state, (expected, now) => {
				const pick = viewerPick(expected as CaptionState | null, now);
				expect(pick === null).toBe(expected === null || expected === now);
				if (pick !== null) expect(pick).toBe(now);
			})
		);
	});
});

describe('noteCaptionChange', () => {
	it("ignores the app's own start, takes the viewer's picks, and nothing once the tracks are going", () => {
		const video = {};
		const list = [sub('showing'), sub('disabled'), sub('disabled')];
		// Tracks attached, nothing set yet: the browser's own selection is
		// no pick.
		expect(noteCaptionChange(video, list)).toBeNull();
		expectCaptions(video, showingCaption(list));
		expect(noteCaptionChange(video, list)).toBeNull();
		// The viewer turns on the third track through the browser's menu.
		list[0].mode = 'disabled';
		list[2].mode = 'showing';
		expect(noteCaptionChange(video, list)).toBe(2);
		// The same state again is no second pick.
		expect(noteCaptionChange(video, list)).toBeNull();
		// Off.
		list[2].mode = 'disabled';
		expect(noteCaptionChange(video, list)).toBe('off');
		// The session's tracks are being removed: whatever changes now is
		// no pick.
		expectCaptions(video, null);
		list[1].mode = 'showing';
		expect(noteCaptionChange(video, list)).toBeNull();
	});
});

describe('extraBeside', () => {
	it('is every other caption track showing while the expected one shows too', () => {
		expect(extraBeside(1, [sub('showing'), sub('showing'), sub('disabled')])).toEqual([0]);
		expect(extraBeside(1, [sub('showing'), sub('showing'), sub('showing')])).toEqual([0, 2]);
		expect(extraBeside(1, [{ kind: 'metadata', mode: 'showing' }, sub('showing')])).toEqual([]);
	});

	it('is nothing when the expected track no longer shows, or the app expects none or off', () => {
		expect(extraBeside(1, [sub('showing'), sub('disabled')])).toEqual([]);
		expect(extraBeside(null, [sub('showing'), sub('showing')])).toEqual([]);
		expect(extraBeside('off', [sub('showing'), sub('showing')])).toEqual([]);
	});

	it('names only caption tracks showing other than the expected one, which shows', () => {
		const track = fc.record({
			kind: fc.constantFrom('subtitles', 'captions', 'metadata'),
			mode: fc.constantFrom('showing', 'hidden', 'disabled')
		});
		fc.assert(
			fc.property(fc.array(track, { maxLength: 8 }), fc.nat(8), (list, expected) => {
				const extra = extraBeside(expected, list);
				const startShows =
					expected < list.length &&
					list[expected].mode === 'showing' &&
					(list[expected].kind === 'subtitles' || list[expected].kind === 'captions');
				if (!startShows) expect(extra).toEqual([]);
				for (const i of extra) {
					expect(i).not.toBe(expected);
					expect(list[i].mode).toBe('showing');
					expect(['subtitles', 'captions']).toContain(list[i].kind);
				}
			})
		);
	});
});

describe('a track the browser turns on beside the one the app expects', () => {
	it('is no pick of the viewer', () => {
		const video = {};
		const list = [sub('disabled'), sub('showing')];
		expectCaptions(video, 1);
		list[0].mode = 'showing';
		expect(noteCaptionChange(video, list)).toBeNull();
		// Switched back off, the captions are as the app set them.
		list[0].mode = 'disabled';
		expect(noteCaptionChange(video, list)).toBeNull();
	});
});
