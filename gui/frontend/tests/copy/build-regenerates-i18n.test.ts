// Pins the packaging invariant, not a string: whatever produces
// shipped output must regenerate the merged locale bundles first.
//
// Lives outside src/ with the other copy-source pins — it asserts on
// repo data (package.json), which is not app source.

import { describe, it, expect } from 'vitest';

import pkg from '../../package.json';

describe('shipped i18n is built from the namespace sources', () => {
	// `messages/<locale>.json` is GENERATED from
	// `messages/<locale>/<ns>.json` by tools/build-messages.mjs, and it
	// is what Paraglide compiles. `dev`, `test` and `check` all run
	// `i18n:compile` first, so working locally always sees fresh copy —
	// but `build` is what the release chain calls
	// (electron `dist:release` -> `build:frontend` -> `pnpm build`).
	//
	// Without this, editing a namespace source changes nothing that
	// ships: the build compiles whatever merged bundle happens to be on
	// disk. Codex caught exactly that — locale sources dropped the
	// yt-dlp recommendation while the shipped English bundle kept it.
	it('the build script regenerates the merged bundles', () => {
		const scripts = pkg.scripts as Record<string, string>;
		expect(scripts.build, 'package.json scripts.build').toContain('i18n:compile');
	});
});
