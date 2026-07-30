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

module.exports = { needsExtraction, assertDepShape };
