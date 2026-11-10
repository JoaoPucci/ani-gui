import { describe, expect, it } from 'vitest';
import { m } from '$lib/paraglide/messages';
import { DOWNLOAD_QUALITIES, downloadQuality, downloadQualityLabel } from './download-quality';

describe('downloadQuality', () => {
	it('is what the settings name', () => {
		for (const q of ['best', '1080', '720', '480', 'worst']) {
			expect(downloadQuality({ download_quality: q })).toBe(q);
		}
	});

	it('is Best when the settings are missing or name none', () => {
		expect(downloadQuality(null)).toBe('best');
		expect(downloadQuality(undefined)).toBe('best');
		expect(downloadQuality({})).toBe('best');
		expect(downloadQuality({ download_quality: '' })).toBe('best');
	});

	it('passes a hand-edited height through for the backend to match or keep the master', () => {
		expect(downloadQuality({ download_quality: '360' })).toBe('360');
	});
});

describe('downloadQualityLabel', () => {
	it('names each choice as Settings labels it', () => {
		for (const q of DOWNLOAD_QUALITIES) {
			expect(downloadQualityLabel(q.key)).toBe(q.label());
		}
	});

	it('names a hand-edited height by itself, and anything else as Best', () => {
		expect(downloadQualityLabel('360')).toBe('360');
		expect(downloadQualityLabel('')).toBe(m.app_quality_best());
		expect(downloadQualityLabel('nonsense')).toBe(m.app_quality_best());
	});
});
