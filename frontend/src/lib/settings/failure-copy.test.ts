import { describe, expect, it } from 'vitest';
import { describeError } from '$lib/play/describe-error';
import { describeSettingsFailure } from './failure-copy';

describe('describeSettingsFailure', () => {
	it('names an invalid settings file, which the user can fix by hand', () => {
		// `config` from settingsGet is config.toml failing to parse —
		// usually a hand edit. "Restart the app" does not fix that.
		const msg = describeSettingsFailure({ kind: 'config', key: 'error.config.parse' });
		expect(msg).toMatch(/config\.toml/);
		expect(msg).not.toBe(describeError({ kind: 'config' }));
	});

	it('names the settings file when it cannot be read or written', () => {
		const msg = describeSettingsFailure({ kind: 'io', key: 'error.io.generic' });
		expect(msg).toMatch(/settings file/i);
		expect(msg).not.toBe(describeError({ kind: 'io' }));
	});

	it('leaves every other failure to the general copy', () => {
		for (const kind of ['network', 'timeout', 'something_new']) {
			expect(describeSettingsFailure({ kind })).toBe(describeError({ kind }));
		}
	});
});
