import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import {
	applyCaptionPick,
	captionModes,
	type CaptionMode,
	type PickableTrack
} from './caption-pick';

const sub = (mode: CaptionMode = 'disabled'): PickableTrack => ({ kind: 'subtitles', mode });

describe('captionModes', () => {
	it('shows the chosen track', () => {
		expect(captionModes([sub(), sub(), sub()], 1)[1]).toBe('showing');
	});

	it('leaves a track already in its mode untouched', () => {
		expect(captionModes([sub('showing')], 0)).toEqual([null]);
	});

	it('answers one entry per track, in order', () => {
		fc.assert(
			fc.property(
				fc.array(fc.constantFrom<CaptionMode>('showing', 'hidden', 'disabled'), {
					maxLength: 40
				}),
				fc.oneof(fc.constant<'off'>('off'), fc.nat(45)),
				(modes, target) => {
					const out = captionModes(
						modes.map((mode) => sub(mode)),
						target
					);
					expect(out).toHaveLength(modes.length);
				}
			)
		);
	});
});

describe('applyCaptionPick', () => {
	it('writes the modes onto the live list', () => {
		const list = [sub('showing'), sub()];
		applyCaptionPick(list, 1);
		expect(list[1].mode).toBe('showing');
		expect(list[0].mode).not.toBe('showing');
	});
});
