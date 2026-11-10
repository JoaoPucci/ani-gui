/**
 * What the external player's command becomes when the user changes the
 * player's type in Settings (mpv, VLC, IINA, Custom).
 *
 * While the command is still the previous type's bare command ("mpv",
 * "vlc", "iina", in any case, with or without ".exe"), it becomes the
 * new type's: a user who never wrote one gets the player they picked.
 * Between mpv, VLC and IINA, a command the user wrote (a full path, a
 * wrapper, a blank, another type's name) is theirs, and is never
 * touched. Custom has no command of its own, so a change to it keeps
 * the command.
 *
 * A blank command runs the type's standard program. So leaving Custom
 * clears a command that is only a standard program's name ("mpv",
 * "vlc", "iina") to blank, and the new type runs its own: "mpv" left
 * over from Custom would otherwise run mpv under the VLC type. A
 * command with a path or flags is kept. This module guesses no path:
 * where a bare name is installed is the backend's lookup
 * (`backend/src/commands/player_locate.rs`).
 */

import type { ExternalPlayerKind } from '$lib/api';

const BARE: Record<ExternalPlayerKind, string | null> = {
	mpv: 'mpv',
	vlc: 'vlc',
	iina: 'iina',
	custom: null
};

/**
 * The command a player type runs by its bare name; null for Custom.
 * It is also what a blank command runs (the backend's
 * `player_program`), so Settings shows it as the field's placeholder.
 */
export function bareCommand(kind: ExternalPlayerKind): string | null {
	return BARE[kind];
}

/** The command after the type changes from `from` to `to`. */
export function commandAfterKindChange(
	command: string,
	from: ExternalPlayerKind,
	to: ExternalPlayerKind
): string {
	const previous = bareCommand(from);
	const next = bareCommand(to);
	if (next === null) return command;
	if (previous === null) return isBareProgramName(command) ? '' : command;
	return isBareProgramName(command) && programName(command) === previous ? next : command;
}

/**
 * Whether a command is only a standard program's name: "mpv", "vlc" or
 * "iina", in any case, with or without ".exe" and surrounding spaces.
 * A path, a command with flags, another program or a blank is not.
 */
export function isBareProgramName(command: string): boolean {
	return Object.values(BARE).includes(programName(command));
}

/** A command as a program name: trimmed, lower case, without ".exe". */
function programName(command: string): string {
	return command
		.trim()
		.toLowerCase()
		.replace(/\.exe$/, '');
}
