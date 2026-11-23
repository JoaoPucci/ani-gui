/**
 * Changing the external player's type in Settings. Not yet built: the
 * red commit's placeholder, which saves nothing.
 */

import type { Config, ExternalPlayerKind } from '$lib/api';

export type AskCommand = (
	command: string,
	from: ExternalPlayerKind,
	to: ExternalPlayerKind
) => Promise<string>;

export function createKindChange(
	ask: AskCommand,
	current: () => Config | null,
	save: (next: Config) => Promise<void>
): (to: ExternalPlayerKind) => Promise<void> {
	void [ask, current, save];
	return async () => {};
}
