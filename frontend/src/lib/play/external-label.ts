/**
 * The label of the play page's external-player item, and the button
 * the Settings hint quotes. It names the player the configured flavour
 * opens ("Open in VLC"), so the user knows where the episode goes
 * before clicking. A Custom flavour runs the user's own command and
 * has no product name, so it says "external player" instead.
 */

import type { ExternalPlayerKind } from '$lib/api';
import { m } from '$lib/paraglide/messages';
import type { Locale } from '$lib/paraglide/runtime';
import { playerKindLabel } from './external-toast';

export function externalOpenLabel(kind: ExternalPlayerKind, locale?: Locale): string {
	const options = locale ? { locale } : undefined;
	if (kind === 'custom') return m.play_external_open_custom({}, options);
	return m.play_external_open_in({ player: playerKindLabel(kind) }, options);
}
