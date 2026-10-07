/**
 * User-facing copy for a failure to read or save the settings
 * (`settingsGet` / `settingsPut`), on the settings page and the detail
 * page's settings strip. `config` is config.toml failing to parse,
 * usually after a hand edit; `io` is the file or its folder refusing a
 * read or write. Both name the file the user would fix. Every other
 * kind gets describeError's sentence.
 */

import { m } from '$lib/paraglide/messages';
import { describeError } from '$lib/play/describe-error';

export function describeSettingsFailure(e: unknown): string {
	const kind =
		typeof e === 'object' && e !== null ? (e as Record<string, unknown>).kind : undefined;
	if (kind === 'config') return m.settings_error_file_invalid();
	if (kind === 'io') return m.settings_error_file_unreadable();
	return describeError(e);
}
