/**
 * The command each external player type runs by its bare name, as
 * Settings shows it. What the command becomes when the type changes is
 * the backend's rule (`backend/src/commands/external_player_kind_change.rs`,
 * asked through `$lib/settings/kind-change.ts`), and where a bare name
 * is installed is the backend's lookup
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
