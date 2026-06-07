import type { Config } from '$lib/api';

/**
 * Persist + runtime-flip a locale change from the Settings page.
 *
 * Order is load-bearing: config goes through `persist` FIRST,
 * then the runtime call flips Paraglide (which writes
 * `localStorage[PARAGLIDE_LOCALE]` and triggers the page reload).
 * If we flipped the runtime first, a mid-flush close would leave
 * localStorage holding the new value but config still on the old
 * one — and the preload's "config wins" policy would silently
 * revert the user's pick on the next launch (the bug this whole
 * `i18n-config-authoritative` change exists to fix).
 *
 * Returns a Promise so callers can `await` to chain UI
 * reactivity (toast, etc.) after both writes complete.
 */
export interface ApplyLocaleDeps {
	persist: (cfg: Config) => Promise<void>;
	setRuntimeLocale: (l: string) => void;
}

export async function applyLocale(
	newLocale: string,
	cfg: Config,
	deps: ApplyLocaleDeps
): Promise<void> {
	await deps.persist({ ...cfg, locale: newLocale });
	deps.setRuntimeLocale(newLocale);
}
