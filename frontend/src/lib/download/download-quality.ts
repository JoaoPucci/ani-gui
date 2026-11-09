/**
 * The download quality: the one quality setting the app keeps, which
 * downloads alone read. The built-in player always starts on Auto with
 * the whole master, and the external players and Syncplay get the
 * master too, so nothing else asks for one.
 *
 * `best` keeps the master (the downloader takes its best level); a
 * height asks the backend for that rendition, or the master when the
 * stream does not list it; `worst` the lowest.
 */

import { m } from '$lib/paraglide/messages';

/** The choices Settings offers, in order. Labels are message
 *  functions, called at render, so a locale switch relabels them. */
export const DOWNLOAD_QUALITIES: ReadonlyArray<{ key: string; label: () => string }> = [
	{ key: 'best', label: m.app_quality_best },
	{ key: '1080', label: () => '1080' },
	{ key: '720', label: () => '720' },
	{ key: '480', label: () => '480' },
	{ key: 'worst', label: m.app_quality_worst }
];

/** The quality a download asks for. */
export function downloadQuality(config: { download_quality?: string } | null | undefined): string {
	return config?.download_quality || 'best';
}

/** The download quality as Settings names it. */
export function downloadQualityLabel(quality: string): string {
	const choice = DOWNLOAD_QUALITIES.find((q) => q.key === quality);
	if (choice) return choice.label();
	// A height hand-edited into the settings file names itself.
	return /^[1-9]\d*$/.test(quality) ? quality : m.app_quality_best();
}
