/**
 * What the external player's command becomes when the user changes the
 * player's type in Settings (mpv, VLC, IINA, Custom).
 *
 * While the command is still the previous type's bare command ("mpv",
 * "vlc", "iina"), it becomes the new type's: a user who never wrote one
 * gets the player they picked. A command the user wrote (a full path, a
 * wrapper, a blank, another type's name) is theirs, and is never
 * touched. Custom has no command of its own, so a change to it or from
 * it keeps the command. No path is guessed and nothing is looked up: on
 * Windows a bare `vlc` may not be found on the PATH, and the user
 * browses to the program, as before.
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
	return previous !== null && next !== null && command === previous ? next : command;
}
