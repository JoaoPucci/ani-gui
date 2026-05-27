/**
 * Top-level check: ask GitHub for the latest release, parse it,
 * compare against the running app's version, return ReleaseInfo
 * when newer (else null).
 *
 * Every failure path collapses to null — fetch error, non-200,
 * malformed JSON, draft, missing fields, equal-or-older tag.
 * Callers can branch on a single nullable.
 *
 * The fetcher is injected so tests can run without stubbing the
 * global. Production wires it to `window.fetch`.
 */

import { parseGitHubRelease, type ReleaseInfo } from './release-parse';
import { isNewerVersion } from './version-compare';

const RELEASES_URL = 'https://api.github.com/repos/JoaoPucci/ani-gui/releases/latest';

export interface CheckForUpdateOptions {
	/** The currently-running app's version (e.g. `0.4.0`). */
	currentVersion: string;
	/** Function with `fetch`'s shape. Inject `window.fetch` in prod. */
	fetcher: typeof fetch;
}

export async function checkForUpdate(opts: CheckForUpdateOptions): Promise<ReleaseInfo | null> {
	let resp: Response;
	try {
		resp = await opts.fetcher(RELEASES_URL, {
			headers: {
				Accept: 'application/vnd.github+json',
				'X-GitHub-Api-Version': '2022-11-28'
			}
		});
	} catch {
		return null;
	}
	if (!resp.ok) return null;
	let body: unknown;
	try {
		body = await resp.json();
	} catch {
		return null;
	}
	const release = parseGitHubRelease(body);
	if (!release) return null;
	if (!isNewerVersion(opts.currentVersion, release.tag)) return null;
	return release;
}
