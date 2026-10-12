// How fast downloads are going, as the download indicators show it.
// The backend reports each running download's speed in bytes a second;
// these turn the running downloads' speeds into one total and that
// total into a unit and a number the message bundles render.

import type { DownloadItem } from './store.svelte';
import { m } from '$lib/paraglide/messages';
import { getLocale } from '$lib/paraglide/runtime';

const MIB = 1024 * 1024;

/** A speed as the indicators show it: whole kilobytes a second under
 *  a mebibyte a second, megabytes to one decimal from there up. */
export function speedParts(bytesPerSecond: number): { unit: 'kbps' | 'mbps'; value: number } {
	const bps = Math.max(0, bytesPerSecond);
	if (bps >= MIB) return { unit: 'mbps', value: Math.round((bps / MIB) * 10) / 10 };
	return { unit: 'kbps', value: Math.round(bps / 1024) };
}

/** The total speed of the downloads still running, a download that
 *  has not reported yet counting as zero; null while none is running. */
export function totalSpeed(items: Pick<DownloadItem, 'status' | 'speed'>[]): number | null {
	const running = items.filter((i) => i.status === 'pending' || i.status === 'active');
	return running.length > 0 ? running.reduce((sum, i) => sum + (i.speed ?? 0), 0) : null;
}

/** A speed as text in the reader's language: `1.5 MB/s`, `512 KB/s`. */
export function formatSpeed(bytesPerSecond: number): string {
	const { unit, value } = speedParts(bytesPerSecond);
	const number = new Intl.NumberFormat(getLocale(), {
		maximumFractionDigits: unit === 'mbps' ? 1 : 0
	}).format(value);
	return unit === 'mbps'
		? m.download_speed_mbps({ value: number })
		: m.download_speed_kbps({ value: number });
}
