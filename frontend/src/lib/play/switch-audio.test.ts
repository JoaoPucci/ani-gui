// The Sub or Dub a switch carries to the picked show's player.

import { beforeEach, describe, expect, it } from 'vitest';
import { __resetSwitchAudioForTests, carrySwitchAudio, takeSwitchAudio } from './switch-audio';

beforeEach(() => __resetSwitchAudioForTests());

describe('the audio a switch carries', () => {
	it('goes to the next player of the picked show, once', () => {
		carrySwitchAudio('13209', 'dub');
		expect(takeSwitchAudio('13209')).toBe('dub');
		expect(takeSwitchAudio('13209')).toBeUndefined();
	});

	it('is dropped by a player of any other show', () => {
		carrySwitchAudio('13209', 'dub');
		expect(takeSwitchAudio('818')).toBeUndefined();
		expect(takeSwitchAudio('13209')).toBeUndefined();
	});

	it('is forgotten by the test seam', () => {
		carrySwitchAudio('13209', 'dub');
		__resetSwitchAudioForTests();
		expect(takeSwitchAudio('13209')).toBeUndefined();
	});
});
