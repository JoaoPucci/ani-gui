import { describe, expect, test, vi } from 'vitest';
import { bootLocaleFromConfig, type BootLocaleDeps } from './boot-locale';

function deps(
	over: Partial<BootLocaleDeps> = {}
): BootLocaleDeps & { setLocale: ReturnType<typeof vi.fn> } {
	return {
		readConfigLocale: () => 'pt-BR',
		locales: ['en', 'pt-BR'],
		getLocale: () => 'en',
		setLocale: vi.fn(),
		...over
	} as BootLocaleDeps & { setLocale: ReturnType<typeof vi.fn> };
}

describe('bootLocaleFromConfig', () => {
	test('flips Paraglide to the locale config.toml names', () => {
		const d = deps();
		bootLocaleFromConfig(d);
		expect(d.setLocale).toHaveBeenCalledWith('pt-BR');
	});

	test('leaves Paraglide alone when there is no bridge or no locale key', () => {
		for (const read of [() => undefined, () => null, () => '']) {
			const d = deps({ readConfigLocale: read });
			bootLocaleFromConfig(d);
			expect(d.setLocale).not.toHaveBeenCalled();
		}
	});

	test('ignores a locale the build does not ship, so Paraglide never throws on it', () => {
		const d = deps({ readConfigLocale: () => 'xx-YY' });
		bootLocaleFromConfig(d);
		expect(d.setLocale).not.toHaveBeenCalled();
	});

	test('does nothing when Paraglide already holds that locale', () => {
		const d = deps({ getLocale: () => 'pt-BR' });
		bootLocaleFromConfig(d);
		expect(d.setLocale).not.toHaveBeenCalled();
	});

	test('treats a getLocale that throws as no locale yet', () => {
		const d = deps({
			getLocale: () => {
				throw new Error('no strategy resolved');
			}
		});
		bootLocaleFromConfig(d);
		expect(d.setLocale).toHaveBeenCalledWith('pt-BR');
	});
});
