import { describe, expect, it, vi } from 'vitest';
import { applyPrimarySelection } from './set-primary';
import type { Config } from '$lib/api';

function cfg(primary = ''): Config {
	return {
		locale: 'en',
		mode: 'sub',
		quality: 'best',
		external_player: 'mpv',
		external_player_kind: 'mpv',
		external_player_custom_args: '',
		syncplay_binary: 'syncplay',
		image_cache_cap_mb: 500,
		auto_play_next: false,
		download_bottom_bar_enabled: true,
		auto_skip_op: false,
		auto_skip_ed: false,
		use_custom_player_controls: false,
		disable_auto_pip_on_leave: false,
		auto_update_anicli: true,
		update_include_prereleases: true,
		primary_account: primary
	};
}

function deps(over: Partial<Parameters<typeof applyPrimarySelection>[2]> = {}) {
	return {
		persist: vi.fn().mockResolvedValue(undefined),
		applyToStore: vi.fn(),
		onError: vi.fn(),
		...over
	};
}

describe('applyPrimarySelection', () => {
	it('no-ops when no config is loaded', async () => {
		const d = deps();
		const out = await applyPrimarySelection(null, 'mal', d);
		expect(out).toBeNull();
		expect(d.persist).not.toHaveBeenCalled();
		expect(d.applyToStore).not.toHaveBeenCalled();
	});

	it('no-ops when the value is unchanged', async () => {
		const d = deps();
		const config = cfg('mal');
		const out = await applyPrimarySelection(config, 'mal', d);
		expect(out).toBe(config);
		expect(d.persist).not.toHaveBeenCalled();
		expect(d.applyToStore).not.toHaveBeenCalled();
	});

	it('persists the new config and updates the store on success', async () => {
		const d = deps();
		const out = await applyPrimarySelection(cfg(''), 'mal', d);
		expect(out?.primary_account).toBe('mal');
		expect(d.persist).toHaveBeenCalledWith(expect.objectContaining({ primary_account: 'mal' }));
		expect(d.applyToStore).toHaveBeenLastCalledWith('mal');
		expect(d.onError).not.toHaveBeenCalled();
	});

	it('rolls the store back to the previous provider and keeps the old config when the write fails', async () => {
		const d = deps({ persist: vi.fn().mockRejectedValue(new Error('disk full')) });
		const config = cfg('anilist');
		const out = await applyPrimarySelection(config, 'mal', d);
		// Page keeps the original config (so re-selecting can retry).
		expect(out).toBe(config);
		// Optimistic update to 'mal' then rolled back to the prior 'anilist'.
		expect(d.applyToStore).toHaveBeenNthCalledWith(1, 'mal');
		expect(d.applyToStore).toHaveBeenLastCalledWith('anilist');
		expect(d.onError).toHaveBeenCalledOnce();
	});

	it('rolls back to null when the previous primary was unset', async () => {
		const d = deps({ persist: vi.fn().mockRejectedValue(new Error('nope')) });
		await applyPrimarySelection(cfg(''), 'mal', d);
		expect(d.applyToStore).toHaveBeenLastCalledWith(null);
	});
});
