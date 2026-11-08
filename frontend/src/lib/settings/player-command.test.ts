// What the external player's command becomes when its type changes.

import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import type { ExternalPlayerKind } from '$lib/api';
import { bareCommand, commandAfterKindChange } from './player-command';

const KINDS: ExternalPlayerKind[] = ['mpv', 'vlc', 'iina', 'custom'];
const kind = fc.constantFrom(...KINDS);
/** Commands a user may have: the bare defaults, paths, wrappers, blanks. */
const command = fc.oneof(
	fc.constantFrom('mpv', 'vlc', 'iina', '', ' mpv ', 'MPV', 'vlc.exe'),
	fc.constantFrom(
		'/usr/bin/mpv',
		'C:\\Program Files\\VideoLAN\\VLC\\vlc.exe',
		'flatpak run io.mpv.Mpv',
		'/Applications/IINA.app/Contents/MacOS/iina-cli'
	),
	fc.string()
);

describe('the bare command of a player type', () => {
	it('is its own name for mpv, VLC and IINA, and none for Custom', () => {
		expect(bareCommand('mpv')).toBe('mpv');
		expect(bareCommand('vlc')).toBe('vlc');
		expect(bareCommand('iina')).toBe('iina');
		expect(bareCommand('custom')).toBeNull();
	});
});

describe('the command after a change of player type', () => {
	it("becomes the new type's bare command while it is the previous type's", () => {
		expect(commandAfterKindChange('mpv', 'mpv', 'vlc')).toBe('vlc');
		expect(commandAfterKindChange('vlc', 'vlc', 'iina')).toBe('iina');
		expect(commandAfterKindChange('iina', 'iina', 'mpv')).toBe('mpv');
	});

	it('is never touched once the user changed it: a path, a wrapper, a blank', () => {
		expect(commandAfterKindChange('/usr/bin/mpv', 'mpv', 'vlc')).toBe('/usr/bin/mpv');
		expect(commandAfterKindChange('flatpak run io.mpv.Mpv', 'mpv', 'iina')).toBe(
			'flatpak run io.mpv.Mpv'
		);
		expect(commandAfterKindChange('', 'mpv', 'vlc')).toBe('');
		// Another type's bare command is the user's choice for this one.
		expect(commandAfterKindChange('vlc', 'mpv', 'iina')).toBe('vlc');
	});

	it('is kept going to Custom or coming from it: Custom has no command of its own', () => {
		expect(commandAfterKindChange('mpv', 'mpv', 'custom')).toBe('mpv');
		expect(commandAfterKindChange('mpv', 'custom', 'vlc')).toBe('mpv');
	});

	it("turns every bare type's own command into every other bare type's", () => {
		const bare = fc.constantFrom<ExternalPlayerKind>('mpv', 'vlc', 'iina');
		fc.assert(
			fc.property(bare, bare, (from, to) => {
				expect(commandAfterKindChange(bareCommand(from)!, from, to)).toBe(bareCommand(to));
			})
		);
	});

	it("is the command as it was, or the new type's bare command, and nothing else", () => {
		fc.assert(
			fc.property(command, kind, kind, (cmd, from, to) => {
				const next = commandAfterKindChange(cmd, from, to);
				expect(next === cmd || next === bareCommand(to)).toBe(true);
			})
		);
	});

	it("changes only a command that is exactly the previous type's bare command", () => {
		fc.assert(
			fc.property(command, kind, kind, (cmd, from, to) => {
				if (commandAfterKindChange(cmd, from, to) !== cmd) {
					expect(cmd).toBe(bareCommand(from));
					expect(bareCommand(to)).not.toBeNull();
				}
			})
		);
	});

	it('comes back to where it started when the type goes back, from a bare command', () => {
		fc.assert(
			fc.property(
				fc.constantFrom<ExternalPlayerKind>('mpv', 'vlc', 'iina'),
				fc.array(kind, { maxLength: 6 }),
				(start, path) => {
					let cmd = bareCommand(start)!;
					let at: ExternalPlayerKind = start;
					for (const next of [...path, start]) {
						cmd = commandAfterKindChange(cmd, at, next);
						at = next;
					}
					// Through Custom the command stays what it was, so only a
					// path that never leaves the bare types is a round trip.
					if (!path.includes('custom')) expect(cmd).toBe(bareCommand(start));
				}
			)
		);
	});
});
