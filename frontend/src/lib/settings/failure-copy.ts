/**
 * User-facing copy for a failure to read or save the settings
 * (`settingsGet` / `settingsPut`), on the settings page and the detail
 * page's settings strip.
 */

import { describeError } from '$lib/play/describe-error';

export function describeSettingsFailure(e: unknown): string {
	return describeError(e);
}
