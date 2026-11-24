// The bare command of each external player type, as Settings shows it.
// What the command becomes when the type changes is the backend's rule,
// held by `backend/src/commands/external_player_kind_change_test.rs`
// and its properties.

import { describe, expect, it } from 'vitest';
import { bareCommand } from './player-command';

describe('the bare command of a player type', () => {
	it('is its own name for mpv, VLC and IINA, and none for Custom', () => {
		expect(bareCommand('mpv')).toBe('mpv');
		expect(bareCommand('vlc')).toBe('vlc');
		expect(bareCommand('iina')).toBe('iina');
		expect(bareCommand('custom')).toBeNull();
	});
});
