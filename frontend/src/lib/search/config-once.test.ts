import { describe, expect, it, vi } from 'vitest';
import type { Config } from '$lib/api';
import { configOnce } from './config-once';

const dub = { mode: 'dub' } as unknown as Config;

describe('configOnce', () => {
	it('loads once, shared by everyone waiting, and keeps what loaded', async () => {
		const load = vi.fn().mockResolvedValue(dub);
		const settings = configOnce(load);
		expect(settings.config).toBeNull();
		await Promise.all([settings.ready(), settings.ready()]);
		await settings.ready();
		expect(load).toHaveBeenCalledTimes(1);
		expect(settings.config).toBe(dub);
	});

	it('loads again after a failure, keeping nothing from it', async () => {
		const load = vi.fn().mockRejectedValueOnce(new Error('down')).mockResolvedValueOnce(dub);
		const settings = configOnce(load);
		await settings.ready();
		expect(settings.config).toBeNull();
		await settings.ready();
		expect(load).toHaveBeenCalledTimes(2);
		expect(settings.config).toBe(dub);
	});
});
