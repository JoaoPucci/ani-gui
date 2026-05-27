/**
 * Lightweight version comparison for the update notifier.
 *
 * Not a full semver implementation — our tags are always plain
 * `vX.Y.Z` (or `X.Y.Z`) and we cut releases manually. The helper
 * splits on `.`, strips a leading `v`, compares each segment as a
 * number, and treats parse failures as "not newer" so a malformed
 * tag never triggers a phantom badge.
 */

function parse(version: string): number[] | null {
	const stripped = version.trim().replace(/^v/i, '');
	if (!stripped) return null;
	const parts = stripped.split('.');
	const nums: number[] = [];
	for (const p of parts) {
		if (!/^\d+$/.test(p)) return null;
		nums.push(Number.parseInt(p, 10));
	}
	return nums;
}

export function isNewerVersion(current: string, candidate: string): boolean {
	const cur = parse(current);
	const cand = parse(candidate);
	if (!cur || !cand) return false;
	const len = Math.max(cur.length, cand.length);
	for (let i = 0; i < len; i++) {
		const c = cur[i] ?? 0;
		const n = cand[i] ?? 0;
		if (n > c) return true;
		if (n < c) return false;
	}
	return false;
}
