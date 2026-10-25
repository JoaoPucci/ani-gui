import { beforeEach, describe, expect, it } from 'vitest';

import { noticeBusy, providerName, reportProgressNotSaved } from './busy-save';
import { toastStore } from '$lib/toasts/store.svelte';
import { m } from '$lib/paraglide/messages';

beforeEach(() => {
	for (const item of [...toastStore.items]) toastStore.dismiss(item.id);
});

describe('providerName', () => {
	it('names each tracker by its display name, and an unnamed one by its id', () => {
		expect(providerName('anilist')).toBe(m.account_provider_anilist());
		expect(providerName('mal')).toBe(m.account_provider_mal());
		expect(providerName('inhouse')).toBe('inhouse');
	});
});

describe('noticeBusy', () => {
	it('stays up for the wait, and no shorter than a plain toast', () => {
		noticeBusy('anilist', 40);
		noticeBusy('mal', 1);
		const [long, short] = toastStore.items;
		expect(long).toMatchObject({
			kind: 'info',
			message: m.account_busy_saving({ provider: m.account_provider_anilist(), seconds: 40 }),
			duration: 40_000
		});
		expect(short.duration).toBe(4000);
	});
});

describe('reportProgressNotSaved', () => {
	it('says which tracker did not take the progress', () => {
		reportProgressNotSaved('anilist');
		expect(toastStore.items).toEqual([
			expect.objectContaining({
				kind: 'error',
				message: m.account_progress_save_failed({ provider: m.account_provider_anilist() })
			})
		]);
	});
});
