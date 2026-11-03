import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import {
	applyCaptionPick,
	captionModes,
	type CaptionMode,
	type CaptionTarget,
	type PickableTrack
} from './caption-pick';

const sub = (mode: CaptionMode = 'disabled'): PickableTrack => ({ kind: 'subtitles', mode });
const metadata = (mode: CaptionMode = 'hidden'): PickableTrack => ({ kind: 'metadata', mode });

const MODES = ['showing', 'hidden', 'disabled'] as const;
const KINDS = ['subtitles', 'captions', 'metadata', 'chapters', 'descriptions'] as const;
const isCaption = (t: PickableTrack) => t.kind === 'subtitles' || t.kind === 'captions';

describe('captionModes', () => {
	it('shows the chosen track', () => {
		expect(captionModes([sub(), sub(), sub()], 1)[1]).toBe('showing');
	});

	it('leaves a track already in its mode untouched', () => {
		expect(captionModes([sub('showing')], 0)).toEqual([null]);
	});

	// A hidden track is still loaded — the browser fetches its file and
	// parses its cues — so setting the unchosen ones aside as hidden
	// made the first pick fetch every track the session listed: 33
	// requests for Steel Ball Run's first episode. Disabled ones are
	// never fetched.
	it('disables every subtitle track it does not show', () => {
		expect(captionModes([sub('showing'), sub('hidden'), sub('disabled'), sub()], 3)).toEqual([
			'disabled',
			'disabled',
			null,
			'showing'
		]);
	});

	it('turning captions off disables every subtitle track', () => {
		expect(captionModes([sub('showing'), sub('hidden'), sub()], 'off')).toEqual([
			'disabled',
			'disabled',
			null
		]);
	});

	// hls.js keeps a metadata track for the stream's ID3 cues; it is
	// not the picker's to show or hide.
	it('leaves a track that is not a subtitle track as it is', () => {
		expect(captionModes([metadata('hidden'), sub(), sub('showing')], 1)).toEqual([
			null,
			'showing',
			'disabled'
		]);
		expect(captionModes([metadata('hidden')], 0)).toEqual([null]);
	});

	it('a choice past the list shows nothing', () => {
		expect(captionModes([sub('showing'), sub()], 7)).toEqual(['disabled', null]);
	});

	it('after any pick, the chosen subtitle track alone shows, every other one is disabled, and other kinds are untouched', () => {
		fc.assert(
			fc.property(
				fc.array(fc.record({ kind: fc.constantFrom(...KINDS), mode: fc.constantFrom(...MODES) }), {
					maxLength: 40
				}),
				fc.oneof(fc.constant<'off'>('off'), fc.nat(45)),
				(tracks: PickableTrack[], target: CaptionTarget) => {
					const before = tracks.map((t) => ({ ...t }));
					const list = tracks.map((t) => ({ ...t }));
					applyCaptionPick(list, target);
					list.forEach((t, i) => {
						if (!isCaption(before[i])) {
							expect(t.mode).toBe(before[i].mode);
						} else if (target === i) {
							expect(t.mode).toBe('showing');
						} else {
							expect(t.mode).toBe('disabled');
						}
					});
					// Only what changes is written.
					captionModes(before, target).forEach((mode, i) => {
						if (mode !== null) expect(mode).not.toBe(before[i].mode);
					});
				}
			)
		);
	});
});

describe('applyCaptionPick', () => {
	it('writes the modes onto the live list', () => {
		const list = [sub('showing'), sub()];
		applyCaptionPick(list, 1);
		expect(list.map((t) => t.mode)).toEqual(['disabled', 'showing']);
	});
});
