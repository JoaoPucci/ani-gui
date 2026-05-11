import { describe, expect, it } from 'vitest';
import { computeToastBottomOffset } from './dock-offset';

describe('computeToastBottomOffset', () => {
	// Pure helper: maps (dock visibility, layout constants) → toast
	// `inset-block-end` so the toast rides above the DownloadBar
	// without overlapping when downloads are in flight.
	const layout = { baseRem: 0.75, dockHeightRem: 3.5, gapRem: 0.75 };

	it('returns the base offset when the dock is hidden', () => {
		const got = computeToastBottomOffset({ dockVisible: false, ...layout });
		expect(got).toBe(0.75);
	});

	it('clears the dock when visible: base + dockHeight + gap', () => {
		const got = computeToastBottomOffset({ dockVisible: true, ...layout });
		expect(got).toBe(0.75 + 3.5 + 0.75);
	});

	it('ignores the dock height when dockVisible is false even if non-zero', () => {
		// Defensive: a stale dockHeightRem reading shouldn't push the
		// toast up when the dock isn't actually on-screen.
		const got = computeToastBottomOffset({
			dockVisible: false,
			baseRem: 0.75,
			dockHeightRem: 12,
			gapRem: 1
		});
		expect(got).toBe(0.75);
	});
});
