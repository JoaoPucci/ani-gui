/**
 * Dock copy for the download failures that mean something different
 * in a download than describeError's general sentence says:
 *
 *   - `timeout`: the backend raises it for the transfer's own one-hour
 *     deadline and for a wait on another transfer's lock as well as for
 *     a slow resolve, with an identical payload, so the copy has to be
 *     true of all three
 *   - `io`: the destination folder refusing the file
 *   - `config`: no download folder set and no default to fall back to
 *   - `scraper`: the download tool exiting non-zero, or (with the key
 *     `error.download.tool_spawn_failed`) not starting at all
 *
 * Every other kind gets describeError's sentence.
 */

import { m } from '$lib/paraglide/messages';
import { describeError } from '$lib/play/describe-error';

const COPY_BY_KIND: Readonly<Record<string, () => string>> = {
	timeout: m.download_failure_timeout,
	io: m.download_failure_write,
	config: m.download_failure_no_folder,
	scraper: m.download_failure_tool
};

export function describeDownloadFailure(e: unknown): string {
	const obj = typeof e === 'object' && e !== null ? (e as Record<string, unknown>) : null;
	const kind = obj?.kind;
	// The download-tool variant carries the spawn key when the tool was
	// found but could not be started at all.
	if (kind === 'scraper' && obj?.key === 'error.download.tool_spawn_failed') {
		return m.download_failure_tool_spawn();
	}
	if (typeof kind === 'string' && Object.hasOwn(COPY_BY_KIND, kind)) return COPY_BY_KIND[kind]();
	return describeError(e);
}
