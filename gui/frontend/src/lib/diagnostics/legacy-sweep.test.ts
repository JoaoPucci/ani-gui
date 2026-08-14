/**
 * The diagnostics page's rendering decision for the boot sweep.
 *
 * Extracted rather than left inline per the rule in AGENTS.md §2: the
 * three input states below each mean something different to the user,
 * and deciding between them inside a `.svelte` `{#if}` would put that
 * judgement somewhere no test can reach.
 */
import { describe, it, expect } from 'vitest';
import { legacySweepView } from './legacy-sweep';

describe('legacySweepView', () => {
	it('shows the block with what was removed', () => {
		// The one launch where it matters: the user's cache held the
		// copy an earlier version maintained, and the app deleted it.
		// Naming the path is the whole point — "we cleaned something
		// up" without saying what is not a report.
		const view = legacySweepView(['/home/u/.cache/ani-gui/ani-cli']);
		expect(view.visible).toBe(true);
		expect(view.paths).toEqual(['/home/u/.cache/ani-gui/ani-cli']);
	});

	it('stays hidden when the sweep removed nothing', () => {
		// Every launch after the first, and every install that never
		// ran a version that kept a copy — which is the overwhelming
		// majority of launches. A permanently-empty diagnostics section
		// is noise, so the block is absent rather than empty.
		expect(legacySweepView([]).visible).toBe(false);
	});

	it('stays hidden when the backend does not report the field', () => {
		// A backend older than this field — in practice a stale debug
		// binary during development, since packaged builds ship both
		// halves together. `undefined` must read as "nothing to say",
		// not crash the page that iterates it.
		expect(legacySweepView(undefined).visible).toBe(false);
		expect(legacySweepView(undefined).paths).toEqual([]);
	});

	it('lists every path when a sweep removed more than one', () => {
		const view = legacySweepView(['/cache/ani-gui/ani-cli', '/cache/ani-gui-dev/ani-cli']);
		expect(view.paths).toHaveLength(2);
	});
});
