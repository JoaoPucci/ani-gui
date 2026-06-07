import { describe, expect, test, vi } from 'vitest';
import type { Config } from '$lib/api';
import { applyLocale } from './apply-locale';

function cfg(): Config {
	return {
		locale: 'en',
		mode: 'sub',
		quality: 'best'
	} as Config;
}

describe('applyLocale', () => {
	test('persists config BEFORE flipping the runtime locale', async () => {
		const calls: string[] = [];
		let releasePersist: () => void = () => {};
		const persistPromise = new Promise<void>((res) => {
			releasePersist = res;
		});
		const persist = vi.fn(async (next: Config) => {
			calls.push(`persist:start:${next.locale}`);
			await persistPromise;
			calls.push(`persist:end:${next.locale}`);
		});
		const setRuntimeLocale = vi.fn((l: string) => {
			calls.push(`runtime:${l}`);
		});

		const run = applyLocale('pt-BR', cfg(), { persist, setRuntimeLocale });
		// Runtime call must NOT have fired yet — persist hasn't resolved.
		expect(calls).toEqual(['persist:start:pt-BR']);
		expect(setRuntimeLocale).not.toHaveBeenCalled();

		releasePersist();
		await run;
		// Order: persist starts, persist ends, runtime flip. A
		// flipped order would let a mid-flush close leave
		// localStorage holding the new locale while config still
		// holds the old one, and the preload's "config wins"
		// overwrite would revert the user's pick on next launch.
		expect(calls).toEqual(['persist:start:pt-BR', 'persist:end:pt-BR', 'runtime:pt-BR']);
	});

	test('passes the new locale through to persist alongside the rest of the config', async () => {
		const persist = vi.fn(async () => {});
		const setRuntimeLocale = vi.fn();
		const base = { ...cfg(), mode: 'dub' as const, quality: 'worst' };
		await applyLocale('ru', base, { persist, setRuntimeLocale });
		expect(persist).toHaveBeenCalledWith({ ...base, locale: 'ru' });
	});

	test('does NOT swallow a persist rejection (runtime flip stays unfired)', async () => {
		const persist = vi.fn().mockRejectedValue(new Error('disk full'));
		const setRuntimeLocale = vi.fn();
		await expect(applyLocale('es-419', cfg(), { persist, setRuntimeLocale })).rejects.toThrow(
			'disk full'
		);
		expect(setRuntimeLocale).not.toHaveBeenCalled();
	});
});
