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

/**
 * Endpoint choice depends on whether we accept pre-releases.
 *
 * `/releases/latest` only surfaces *full* releases — prerelease=false,
 * draft=false. ani-gui's cuts so far are all marked prerelease=true,
 * so it returns 404 in this codebase today. Hitting it makes sense
 * once stable releases start landing.
 *
 * `/releases?per_page=1` returns an array of the single newest
 * release (including pre-releases). Drafts are filtered for
 * unauthenticated callers, which is what we want.
 */
const RELEASES_LIST_URL = 'https://api.github.com/repos/JoaoPucci/ani-gui/releases?per_page=1';
const RELEASES_LATEST_URL = 'https://api.github.com/repos/JoaoPucci/ani-gui/releases/latest';

export interface CheckForUpdateOptions {
	/** The currently-running app's version (e.g. `0.4.0`). */
	currentVersion: string;
	/** Function with `fetch`'s shape. Inject `window.fetch` in prod. */
	fetcher: typeof fetch;
	/** Whether to consider pre-releases when looking for an update.
	 *  Defaults to true because every ani-gui release so far is
	 *  marked prerelease=true; flip to false once stable cuts begin
	 *  if the user wants to ignore release candidates. */
	includePrereleases?: boolean;
}

export async function checkForUpdate(opts: CheckForUpdateOptions): Promise<ReleaseInfo | null> {
	const includePrereleases = opts.includePrereleases ?? true;
	const url = includePrereleases ? RELEASES_LIST_URL : RELEASES_LATEST_URL;
	let resp: Response;
	try {
		resp = await opts.fetcher(url, {
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
	// List endpoint returns an array, latest endpoint a single object.
	const top = Array.isArray(body) ? body[0] : body;
	const release = parseGitHubRelease(top);
	if (!release) return null;
	if (!isNewerVersion(opts.currentVersion, release.tag)) return null;
	return release;
}
