import { describe, expect, it } from 'vitest';
import type { LaunchFlags } from '$lib/flags';
import { CENTER_FLAG, centerEnabled } from './enabled';

/** A launch's flags, typed loosely so a test can hand the check a
 *  value only a broken answer would carry. */
const loose = (flags: Record<string, unknown>) => flags as unknown as LaunchFlags;

describe('the notification center flag', () => {
	it('is named as the backend would declare it', () => {
		expect(CENTER_FLAG).toBe('notification_center');
	});

	it('is off unless the launch flags say true', () => {
		expect(centerEnabled({})).toBe(false);
		expect(centerEnabled({ notification_center: false })).toBe(false);
		expect(centerEnabled(loose({ notification_center: 'true' }))).toBe(false);
		expect(centerEnabled({ notification_center: true })).toBe(true);
	});

	it('reads the window launch flags by default, off when there are none', () => {
		expect(centerEnabled()).toBe(false);
	});
});
