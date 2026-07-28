// Pins the packaging invariant, not a string: whatever produces
// shipped output must regenerate the merged locale bundles first.
//
// Lives outside src/ with the other copy-source pins — it asserts on
// repo data (package.json), which is not app source.

import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, it, expect } from 'vitest';

import pkg from '../../package.json';

const WORKFLOWS = fileURLToPath(new URL('../../../../.github/workflows', import.meta.url));

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

	// Same invariant one layer down. `messages/<locale>.json` is not
	// committed, so anything that runs the suite against a fresh
	// checkout without compiling first gets zero message functions and
	// dies on the first `m.<key>()` call. Every script that reaches
	// vitest has to regenerate, not just the ones that happen to today.
	it('every script that runs vitest regenerates first', () => {
		const scripts = pkg.scripts as Record<string, string>;
		const runners = Object.entries(scripts).filter(([, cmd]) => /\bvitest\b/.test(cmd));
		expect(runners.length, 'no script runs vitest?').toBeGreaterThan(0);
		for (const [name, cmd] of runners) {
			expect(cmd, `package.json scripts.${name}`).toContain('i18n:compile');
		}
	});

	// And one layer further out: a workflow step that calls the binary
	// directly bypasses the scripts entirely. This is how the coverage
	// jobs broke — `pnpm exec vitest run --coverage` never touches
	// `i18n:compile`, so untracking the generated bundle turned a green
	// job red. CI has to go through the scripts that regenerate.
	it('no workflow invokes the vitest binary directly', () => {
		const offenders: string[] = [];
		for (const file of readdirSync(WORKFLOWS).filter((f) => /\.ya?ml$/.test(f))) {
			const lines = readFileSync(join(WORKFLOWS, file), 'utf8').split('\n');
			lines.forEach((line, i) => {
				// `run:` steps only — `name:` labels may say "vitest".
				if (/^\s*run:.*\bvitest\b/.test(line) && !line.includes('i18n:compile')) {
					offenders.push(`${file}:${i + 1}: ${line.trim()}`);
				}
			});
		}
		expect(offenders, 'workflow steps calling vitest without compiling i18n').toEqual([]);
	});
});
