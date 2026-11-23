/**
 * Changing the external player's type in Settings (mpv, VLC, IINA,
 * Custom). What the command becomes is the backend's rule
 * (`backend/src/commands/external_player_kind_change.rs`): the page
 * asks it, then saves the new type and that command together.
 *
 * Changes are taken one at a time, in the order they were made, so a
 * quick second change is asked about from where the first one left the
 * command. A question that fails changes nothing, and `onFail` hears of
 * it: the new type saved with the old command is the state the rule
 * exists to prevent. A command the user edits while the answer is on its way is
 * theirs, and the answer does not replace it.
 */

import type { Config, ExternalPlayerKind } from '$lib/api';

/** Asks the backend what `command` becomes going from `from` to `to`. */
export type AskCommand = (
	command: string,
	from: ExternalPlayerKind,
	to: ExternalPlayerKind
) => Promise<string>;

/**
 * A change of type for the config `current` returns, saved through
 * `save`; `onFail` hears of a question that failed. Each call's
 * promise settles once its change is saved.
 */
export function createKindChange(
	ask: AskCommand,
	current: () => Config | null,
	save: (next: Config) => Promise<void>,
	onFail: (error: unknown) => void = () => {}
): (to: ExternalPlayerKind) => Promise<void> {
	let queue: Promise<void> = Promise.resolve();
	return (to) => {
		const run = queue.then(() => change(to));
		queue = run.catch(() => {});
		return run;
	};

	async function change(to: ExternalPlayerKind): Promise<void> {
		const before = current();
		if (!before) return;
		let command: string;
		try {
			command = await ask(before.external_player, before.external_player_kind, to);
		} catch (e) {
			// Unanswered: the new type with the old command is what the
			// rule prevents, so nothing changes, and the page says so.
			onFail(e);
			return;
		}
		const now = current() ?? before;
		if (now.external_player !== before.external_player) command = now.external_player;
		await save({ ...now, external_player_kind: to, external_player: command });
	}
}
