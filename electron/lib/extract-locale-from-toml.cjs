'use strict';

/**
 * Pull the top-level `locale = "..."` string out of a config.toml
 * dump. Returns the value (as written, may be empty) or null when
 * absent.
 *
 * Why a regex instead of pulling in a full TOML parser: the Electron
 * main process needs this synchronously at app start, before the
 * BrowserWindow exists, so the Paraglide localStorage key can be
 * seeded in the preload. The only field we care about is `locale`;
 * a full parser would add a dependency for one line of value.
 *
 * The regex:
 *   ^ ... locale = "..."  with optional surrounding whitespace,
 *   multiline-anchored so it only matches at line start.
 * We also walk forward from the match to make sure no TOML table
 * header (`[...]`) precedes it — nested `locale` keys belong to that
 * section's data, not to the top-level config.
 */
function extractLocaleFromToml(tomlText) {
	if (typeof tomlText !== 'string' || tomlText.length === 0) return null;
	const re = /^[ \t]*locale[ \t]*=[ \t]*"([^"]*)"/gm;
	let match;
	while ((match = re.exec(tomlText)) !== null) {
		// Reject this match if a table header appears earlier in the
		// file (between start-of-input and the matched line). The
		// top-level `locale` always lands before any `[...]` line —
		// once we cross a header we're inside another table.
		const preface = tomlText.slice(0, match.index);
		if (!/^[ \t]*\[/m.test(preface)) {
			return match[1];
		}
		// Nested `locale` under some `[section]` — keep scanning in
		// case the top-level one shows up further down (TOML doesn't
		// require keys before tables, but it's the common shape).
	}
	return null;
}

module.exports = { extractLocaleFromToml };
