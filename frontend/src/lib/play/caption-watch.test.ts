import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import {
	expectCaptions,
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
