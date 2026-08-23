import { setLocale, getLocale, locales } from '$lib/paraglide/runtime';

/**
 * Flip Paraglide to the locale main.js read from `config.toml`,
 * exposed by the Electron preload as `window.aniGui.configLocale`.
 * SvelteKit runs `hooks.client.ts` once at module init, before any
 * route module imports, so the call lands before the first `m.foo()`
 * is evaluated — Paraglide caches `_locale` after first read, so
 * setting it here pins the value for the whole session.
 *
 * Previous design relayed the locale through localStorage (preload
 * wrote `PARAGLIDE_LOCALE`, Paraglide's strategy read it). That
 * had two failure modes: a stale localStorage value out-voting a
 * fresh config edit, and Paraglide reading something other than
 * localStorage at boot. Replacing it with a direct `setLocale` call
 * removes both — config.toml is the single source of truth, no
 * relay, no race.
 *
 * `{ reload: false }` because we're running BEFORE any render —
 * there is nothing to reload yet, and Paraglide will see the new
 * locale on its first `getLocale()` call.
 *
 * Falls through when:
 *   - `window.aniGui` is missing (web preview, missing preload)
 *   - `configLocale` is null (no config.toml or no `locale` key)
 *   - the value isn't in `locales` (config drift / typo) — guarded
 *     so Paraglide doesn't throw on a setLocale to an unknown locale
 */
function bootLocaleFromConfig(): void {
	if (typeof window === 'undefined') return;
	const aniGui = (window as unknown as { aniGui?: { getConfigLocale?: () => string | null } })
		.aniGui;
	// Sync IPC: re-reads config.toml on every renderer load (not a
	// cached snapshot from window-creation time). That's what lets a
	// Settings change persist across the Paraglide reload — main.js
	// gives us the fresh value, hooks flips Paraglide to match, page
	// renders the user's pick.
	const configLocale = aniGui?.getConfigLocale?.();
	if (!configLocale) return;
	if (!(locales as readonly string[]).includes(configLocale)) return;
	let current: string | null = null;
	try {
		current = getLocale();
	} catch {
		/* getLocale throws when no strategy resolved; treat as un-set */
	}
	if (current === configLocale) return;
	setLocale(configLocale as (typeof locales)[number], { reload: false });
}

bootLocaleFromConfig();
