/**
 * Lightweight version comparison for the update notifier.
 *
 * Not a full semver implementation — our tags are always plain
 * `vX.Y.Z` or `vX.Y.Z-<pre>` (e.g. `v0.5.0-rc1`, `v0.4.0-beta.1`).
 * The parser splits on `.`, strips a leading `v`, peels off any
 * pre-release suffix after the first `-`, and compares each core
 * segment numerically. Parse failures collapse to "not newer" so
 * a malformed tag in the GitHub response never triggers a phantom
 * badge.
 *
 * Pre-release ordering rules (matching semver):
 *   - Same core, both stable → not newer.
 *   - Same core, candidate has pre, current stable → NOT newer
 *     (rc is older than stable).
 *   - Same core, current has pre, candidate stable → newer
 *     (stable wins over rc).
 *   - Same core, both have pre → lexical compare of the suffix.
 *
 * Lexical pre comparison is good enough for our tag conventions
 * (rc1/rc2/rc3, beta.1/beta.2). True semver would require
 * dot-separated identifier comparison; not worth the extra code
 * until our tags need it.
 */

interface ParsedVersion {
	core: number[];
	pre: string | null;
}

function parse(version: string): ParsedVersion | null {
	const stripped = version.trim().replace(/^v/i, '');
	if (!stripped) return null;
	const dashAt = stripped.indexOf('-');
	const corePart = dashAt >= 0 ? stripped.slice(0, dashAt) : stripped;
	const pre = dashAt >= 0 ? stripped.slice(dashAt + 1) : null;
	if (pre !== null && pre.length === 0) return null;
	const parts = corePart.split('.');
	const nums: number[] = [];
	for (const p of parts) {
		if (!/^\d+$/.test(p)) return null;
		nums.push(Number.parseInt(p, 10));
	}
	return { core: nums, pre };
}

export function isNewerVersion(current: string, candidate: string): boolean {
	const cur = parse(current);
	const cand = parse(candidate);
	if (!cur || !cand) return false;
	const len = Math.max(cur.core.length, cand.core.length);
	for (let i = 0; i < len; i++) {
		const c = cur.core[i] ?? 0;
		const n = cand.core[i] ?? 0;
		if (n > c) return true;
		if (n < c) return false;
	}
	// Core versions are equal — decide via pre-release suffix.
	if (cur.pre === null && cand.pre === null) return false;
	if (cur.pre === null) {
		// Candidate has a pre but current is stable — rc is older.
		return false;
	}
	if (cand.pre === null) {
		// Current has a pre, candidate is stable — stable wins.
		return true;
	}
	return cand.pre > cur.pre;
}
