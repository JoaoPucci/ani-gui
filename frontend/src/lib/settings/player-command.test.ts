// What the external player's command becomes when its type changes.

import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import type { ExternalPlayerKind } from '$lib/api';
import { bareCommand, commandAfterKindChange, isBareProgramName } from './player-command';

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

describe('a bare program name', () => {
	it("is a standard program's name alone, in any case, with or without .exe", () => {
		for (const name of ['mpv', 'vlc', 'iina', 'VLC', ' mpv ', 'mpv.exe', 'IINA.EXE']) {
			expect(isBareProgramName(name), name).toBe(true);
		}
	});

	it('is not a path, a command with flags, another program or a blank', () => {
		for (const name of [
			'',
			'  ',
			'/usr/bin/mpv',
			'C:\\mpv\\mpv.exe',
			'mpv --fs',
			'flatpak run io.mpv.Mpv',
			'celluloid',
			'mpvx',
			'vlc.app'
		]) {
			expect(isBareProgramName(name), name).toBe(false);
		}
	});
});

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

	it('is kept going to Custom: Custom has no command of its own', () => {
		expect(commandAfterKindChange('mpv', 'mpv', 'custom')).toBe('mpv');
		expect(commandAfterKindChange('/usr/bin/vlc', 'vlc', 'custom')).toBe('/usr/bin/vlc');
	});

	it("clears a standard program's bare name leaving Custom, so the new type runs its own", () => {
		// A blank command runs the type's standard program (Settings
		// shows it as the placeholder), so "mpv" left over from Custom
		// would otherwise run mpv under the VLC type.
		expect(commandAfterKindChange('mpv', 'custom', 'vlc')).toBe('');
		expect(commandAfterKindChange('vlc', 'custom', 'vlc')).toBe('');
		expect(commandAfterKindChange('iina', 'custom', 'mpv')).toBe('');
		// The same name as typed on Windows, or with stray case or spaces.
		expect(commandAfterKindChange(' VLC ', 'custom', 'mpv')).toBe('');
		expect(commandAfterKindChange('mpv.exe', 'custom', 'iina')).toBe('');
	});

	it('keeps a command the user wrote leaving Custom: a path, flags, a wrapper', () => {
		for (const written of [
			'/usr/bin/mpv',
			'C:\\Program Files\\VideoLAN\\VLC\\vlc.exe',
			'mpv --fs',
			'flatpak run io.mpv.Mpv',
			'celluloid',
			''
		]) {
			expect(commandAfterKindChange(written, 'custom', 'vlc')).toBe(written);
		}
	});

	it("turns every bare type's own command into every other bare type's", () => {
		const bare = fc.constantFrom<ExternalPlayerKind>('mpv', 'vlc', 'iina');
		fc.assert(
			fc.property(bare, bare, (from, to) => {
				expect(commandAfterKindChange(bareCommand(from)!, from, to)).toBe(bareCommand(to));
			})
		);
	});

	it("is the command as it was, the new type's bare command, or blank, and nothing else", () => {
		fc.assert(
			fc.property(command, kind, kind, (cmd, from, to) => {
				const next = commandAfterKindChange(cmd, from, to);
				expect(next === cmd || next === bareCommand(to) || next === '').toBe(true);
			})
		);
	});

	it("changes only the previous type's exact bare command, or a bare name leaving Custom", () => {
		fc.assert(
			fc.property(command, kind, kind, (cmd, from, to) => {
				const next = commandAfterKindChange(cmd, from, to);
				if (next === cmd) return;
				expect(to).not.toBe('custom');
				if (from === 'custom') {
					expect(next).toBe('');
					expect(isBareProgramName(cmd)).toBe(true);
				} else {
					expect(cmd).toBe(bareCommand(from));
					expect(next).toBe(bareCommand(to));
				}
			})
		);
	});

	it('leaves Custom with a blank exactly when the command is a bare name', () => {
		const bare = fc
			.tuple(
				fc.constantFrom('mpv', 'vlc', 'iina', 'MPV', 'Vlc', 'IINA'),
				fc.constantFrom('', '.exe', '.EXE'),
				fc.constantFrom('', ' ', '  ')
			)
			.map(([name, ext, pad]) => `${pad}${name}${ext}${pad}`);
		const standard = fc.constantFrom<ExternalPlayerKind>('mpv', 'vlc', 'iina');
		fc.assert(
			fc.property(bare, standard, (cmd, to) => {
				expect(commandAfterKindChange(cmd, 'custom', to)).toBe('');
			})
		);
		fc.assert(
			fc.property(command, standard, (cmd, to) => {
				const next = commandAfterKindChange(cmd, 'custom', to);
				expect(next).toBe(isBareProgramName(cmd) ? '' : cmd);
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
