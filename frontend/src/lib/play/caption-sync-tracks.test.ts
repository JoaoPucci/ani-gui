import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import { engineIdOf } from './caption-engine';
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

	const all = [{ name: 'English', lang: 'en' }, ...renditions];

	it("takes the engine's pick in the new group for a rendition of the group left", () => {
		expect(followGroup(video([], [oldEn, newEn]), oldEn, renditions, all, 0)).toBe(newEn);
	});

	it('keeps a track no rendition ever named', () => {
		const cc = track('English CC', 'en', 'captions');
		expect(followGroup(video([], [cc, newEn]), cc, renditions, all, 0)).toBe(cc);
	});

	it('for any pick, moves only off a rendition of a group left, and only to the track the engine picked', () => {
		const names = ['English', 'English (CC)', 'Español', 'Français'];
		const langs = ['en', 'es', 'fr'];
		const t = fc.record({
			kind: fc.constantFrom('subtitles', 'captions'),
			label: fc.constantFrom(...names),
			language: fc.constantFrom(...langs),
			mode: fc.constant<'disabled'>('disabled')
		});
		const r = fc.record({ name: fc.constantFrom(...names), lang: fc.constantFrom(...langs) });
		fc.assert(
			fc.property(
				fc.array(t, { maxLength: 4 }),
				fc.array(t, { maxLength: 4 }),
				fc.array(r, { maxLength: 3 }),
				fc.array(r, { maxLength: 3 }),
				fc.integer({ min: -1, max: 4 }),
				fc.integer({ min: 0, max: 8 }),
				(elements, made, current, left, picked, at) => {
					const v = video(elements, made);
					const list = [...elements, ...made];
					const everyone = [...left, ...current];
					for (const desired of [list[at] ?? 'off', 'off', 'own', null] as const) {
						const out = followGroup(v, desired, current, everyone, picked);
						if (out === desired) continue;
						// Only a track on the video, not a sidecar track, that a
						// rendition of a group left names and none of the current
						// group does, moves.
						expect(typeof desired).toBe('object');
						const d = desired as SyncTrack;
						expect(elements).not.toContain(d);
						expect(current.some((c) => engineIdOf(d, [c]) === 0)).toBe(false);
						expect(everyone.some((c) => engineIdOf(d, [c]) === 0)).toBe(true);
						// To the engine's pick's own track.
						expect(out).toBe(list[renditionIndex(list, current[picked], new Set(elements))]);
					}
				}
			)
		);
	});

	it('keeps the pick when it is Off, a sidecar track, a rendition still listed, or the engine picked none', () => {
		const side = track('Français', 'fr');
		const v = video([side], [oldEn, newEn]);
		expect(followGroup(v, 'off', renditions, all, 0)).toBe('off');
		expect(followGroup(v, null, renditions, all, 0)).toBeNull();
		expect(followGroup(v, side, renditions, all, 0)).toBe(side);
		expect(followGroup(v, newEn, renditions, all, 0)).toBe(newEn);
		expect(followGroup(v, oldEn, renditions, all, -1)).toBe(oldEn);
	});

	it("keeps the pick when the engine's new track is not on the video", () => {
		expect(followGroup(video([], [oldEn]), oldEn, renditions, all, 0)).toBe(oldEn);
	});
});
