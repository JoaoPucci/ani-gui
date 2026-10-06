import { setLocale, getLocale, locales } from '$lib/paraglide/runtime';
import { bootLocaleFromConfig } from '$lib/settings/boot-locale';

// Pins the session's locale to config.toml's before the first render.
// The decision is `bootLocaleFromConfig`'s (and unit-tested there);
// this file only wires it to the preload bridge and Paraglide, since
// SvelteKit runs `hooks.client.ts` once at module init, ahead of any
// route module.
if (typeof window !== 'undefined') {
	const aniGui = (window as unknown as { aniGui?: { getConfigLocale?: () => string | null } })
		.aniGui;
	bootLocaleFromConfig({
		readConfigLocale: () => aniGui?.getConfigLocale?.(),
		locales,
		getLocale,
		setLocale: (locale) => setLocale(locale as (typeof locales)[number], { reload: false })
	});
}
