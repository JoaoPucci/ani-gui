import { describe, expect, it } from 'vitest';
import {
	followGroup,
	indexIn,
	isCaption,
	renditionIndex,
	wantOf,
	type CaptionVideo,
	type SyncTrack
} from './caption-sync-tracks';

const track = (label: string, language: string, kind = 'subtitles'): SyncTrack => ({
	kind,
	label,
	language,
	mode: 'disabled'
});

/** A video listing `elements`' tracks, then `made`. */
function video(elements: SyncTrack[], made: SyncTrack[]): CaptionVideo {
	return {
		textTracks: [...elements, ...made],
		querySelectorAll: () => elements.map((t) => ({ track: t }))
	};
}

describe('isCaption', () => {
	it('takes subtitles and captions tracks, nothing else', () => {
		expect(isCaption(track('a', 'en', 'captions'))).toBe(true);
		expect(isCaption(track('a', 'en', 'metadata'))).toBe(false);
	});
});

describe('indexIn', () => {
	it('is -1 for a track not on the list', () => {
		expect(indexIn([track('a', 'en')], track('a', 'en'))).toBe(-1);
	});
});

describe('renditionIndex', () => {
	it("finds the engine's own track, not a sidecar track named alike", () => {
		const side = track('English', 'en');
		const own = track('English', 'en');
		expect(renditionIndex([side, own], { name: 'English', lang: 'en' }, new Set([side]))).toBe(1);
	});

	it('is -1 for no rendition, or one no track is named by', () => {
		expect(renditionIndex([track('a', 'en')], undefined, new Set())).toBe(-1);
		expect(renditionIndex([track('a', 'en')], { name: 'b', lang: 'en' }, new Set())).toBe(-1);
	});
});

describe('wantOf', () => {
	it('asks for none for the app’s own tracks, and Off for a track no longer listed', () => {
		const t = track('English', 'en');
		expect(wantOf('own', -1, new Set(), [])).toBe(-1);
		expect(wantOf(t, -1, new Set(), [])).toBe('off');
		expect(wantOf(t, 0, new Set([t]), [{ name: 'English', lang: 'en' }])).toBe(-1);
		expect(wantOf(t, 0, new Set(), [{ name: 'English', lang: 'en' }])).toBe(0);
	});
});

describe('followGroup', () => {
	const oldEn = track('English', 'en');
	const newEn = track('English (CC)', 'en');
	const renditions = [{ name: 'English (CC)', lang: 'en' }];

	it("takes the engine's pick in the new group for a rendition of the group left", () => {
		expect(followGroup(video([], [oldEn, newEn]), oldEn, renditions, renditions, 0)).toBe(newEn);
	});

	it('keeps the pick when it is Off, a sidecar track, a rendition still listed, or the engine picked none', () => {
		const side = track('Français', 'fr');
		const v = video([side], [oldEn, newEn]);
		expect(followGroup(v, 'off', renditions, renditions, 0)).toBe('off');
		expect(followGroup(v, null, renditions, renditions, 0)).toBeNull();
		expect(followGroup(v, side, renditions, renditions, 0)).toBe(side);
		expect(followGroup(v, newEn, renditions, renditions, 0)).toBe(newEn);
		expect(followGroup(v, oldEn, renditions, renditions, -1)).toBe(oldEn);
	});

	it("keeps the pick when the engine's new track is not on the video", () => {
		expect(followGroup(video([], [oldEn]), oldEn, renditions, renditions, 0)).toBe(oldEn);
	});
});
