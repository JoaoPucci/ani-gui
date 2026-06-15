/**
 * Apply a primary-tracker selection: optimistically update the shared
 * store, persist the new config, and report the config the caller
 * should adopt. Extracted from /account's `setPrimary` so the
 * orchestration (guard / optimistic update / persist / rollback) is
 * unit-testable instead of living in a `.svelte` handler.
 *
 * Returns the config the page should hold afterwards: the new config
 * on success, or the unchanged original on a no-op / failed write.
 */

import type { Config } from '$lib/api';
import type { Provider } from './types';
import { parsePrimaryProvider } from './chip-descriptor';

export interface ApplyPrimaryDeps {
	/** Persist the full config (settings_put is a full round-trip). */
	persist: (cfg: Config) => Promise<void>;
	/** Push the coerced provider into the shared reactive store. */
	applyToStore: (provider: Provider | null) => void;
	/** Surface a save failure to the user (e.g. a toast). */
	onError: () => void;
}

export async function applyPrimarySelection(
	config: Config | null,
	value: string,
	deps: ApplyPrimaryDeps
): Promise<Config | null> {
	// No config loaded yet, or the choice didn't change — nothing to do.
	if (!config || config.primary_account === value) return config;
	const next = { ...config, primary_account: value };
	// Optimistic: reflect the choice in the chip + rail immediately.
	deps.applyToStore(parsePrimaryProvider(value));
	try {
		await deps.persist(next);
		return next;
	} catch {
		deps.onError();
		return config;
	}
}
