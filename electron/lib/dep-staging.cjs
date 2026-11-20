// Shape rules for a bundled-dependency entry, shared by the Linux and
// Windows fetch scripts.
//
// Both scripts declare their deps as data and walk the list. Keeping
// the rules here means a typo fails immediately, by name, instead of
// surfacing several MB into a download as tar or unzip looking for a
// path nobody declared.

/**
 * Whether this dep's binary has to be dug out of an archive.
 *
 * `directBinary` deps are the ones whose upstream publishes the
 * executable itself as the release asset — the verified download IS
 * the binary, so there is nothing to extract.
 */
function needsExtraction(dep) {
	return dep.directBinary !== true;
}

/**
 * Reject a dep that cannot be staged, before any network work starts.
 *
 * Two ways to be unstageable, and they are opposites: an archive dep
 * with no path to its binary, and a direct binary that names one
 * anyway. The second matters because only one of the two can be
 * honoured, so the entry does not say what its author meant.
 */
function assertDepShape(dep) {
	assertOnlyOnShape(dep);
	if (dep.directBinary === true) {
		if (dep.archivePath) {
			throw new Error(
				`dep '${dep.name}': directBinary is set but archivePath is too — ` +
					`only one can be honoured, so drop whichever is wrong`,
			);
		}
		return;
	}
	if (!dep.archivePath) {
		throw new Error(
			`dep '${dep.name}': needs archivePath (the binary's path inside ` +
				`${dep.archiveName}), or directBinary: true if the download is the binary`,
		);
	}
}

/** The packaged platforms, by Node's `process.platform` names. */
const PLATFORM_LABELS = { linux: 'Linux', win32: 'Windows' };

/**
 * Reject a malformed one-platform declaration.
 *
 * `onlyOn` exempts an entry from the parity the two fetchers are held
 * to, so it has to say which platform and why: an exemption without a
 * reason is indistinguishable from the accident the parity check
 * exists to catch.
 */
function assertOnlyOnShape(dep) {
	if (dep.onlyOn === undefined) return;
	const { onlyOn } = dep;
	if (typeof onlyOn !== 'object' || onlyOn === null || !Object.hasOwn(PLATFORM_LABELS, onlyOn.platform)) {
		throw new Error(
			`dep '${dep.name}': onlyOn must be { platform, reason } with platform one of ` +
				Object.keys(PLATFORM_LABELS).join(', '),
		);
	}
	if (typeof onlyOn.reason !== 'string' || !onlyOn.reason.trim()) {
		throw new Error(`dep '${dep.name}': onlyOn needs a reason the dependency is one-platform`);
	}
}

/**
 * Where the two fetchers' dependency sets disagree, as messages; empty
 * when they agree.
 *
 * A dependency staged by one fetcher alone is a gap unless every entry
 * of it on that side declares `onlyOn` naming that side's platform. A
 * declaration naming the other platform contradicts the fetcher it
 * sits in, and one on a dependency both fetchers stage has outlived
 * its reason and would hide the other platform dropping it — both are
 * gaps too. Identity is the declared `dep` field, never a name's shape.
 */
function parityGaps(linuxDeps, windowsDeps) {
	const sides = [
		{ platform: 'linux', deps: linuxDeps, other: windowsDeps },
		{ platform: 'win32', deps: windowsDeps, other: linuxDeps },
	];
	const gaps = [];
	for (const { platform, deps, other } of sides) {
		const label = PLATFORM_LABELS[platform];
		const otherLabel = PLATFORM_LABELS[platform === 'linux' ? 'win32' : 'linux'];
		const otherSet = new Set(other.map((d) => d.dep));
		const byDep = new Map();
		for (const d of deps) byDep.set(d.dep, [...(byDep.get(d.dep) || []), d]);
		for (const [dep, entries] of byDep) {
			const declared = entries.filter((e) => e.onlyOn !== undefined);
			const contradicted = declared.filter((e) => e.onlyOn.platform !== platform);
			for (const e of contradicted) {
				gaps.push(`${dep} is declared onlyOn ${e.onlyOn.platform} but staged by the ${label} fetcher`);
			}
			if (otherSet.has(dep)) {
				if (declared.length) {
					gaps.push(`${dep} is declared one-platform but both fetchers stage it`);
				}
				continue;
			}
			// A contradiction is already reported; an exemption needs every
			// entry of the dependency to carry it.
			if (!contradicted.length && declared.length !== entries.length) {
				gaps.push(
					`${dep} is staged for ${label} but not for ${otherLabel} — a runtime dependency ` +
						`that lands on one packaged platform is unfinished unless every entry of it ` +
						`declares onlyOn with the reason`,
				);
			}
		}
	}
	return gaps;
}

/**
 * Binaries sitting in a staging directory that the current `DEPS` no
 * longer declares.
 *
 * Staging is per-entry: each dep writes the one file it owns and
 * touches nothing else. That is fine while the set only grows, and
 * wrong the moment it shrinks — `extraResources` copies the whole
 * directory into the package, so a retired binary keeps shipping from
 * any checkout that staged it before, with no build-time signal.
 *
 * Keyed on `binary` rather than `dep`, because several entries can
 * share a `dep`: Linux stages the impersonate binary plus a wrapper
 * per browser out of one archive, and each is a file in its own
 * right.
 */
function retiredStagedBinaries(stagedNames, deps) {
	const declared = new Set(deps.map((d) => d.binary));
	return stagedNames.filter((name) => !declared.has(name));
}

module.exports = { needsExtraction, assertDepShape, retiredStagedBinaries, parityGaps };
