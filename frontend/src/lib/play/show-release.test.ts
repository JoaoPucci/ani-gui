import { describe, expect, it } from 'vitest';
import { createShowReleases } from './show-release';

function harness() {
	const released: string[] = [];
	return { released, releases: createShowReleases((id) => released.push(id)) };
}

describe('createShowReleases', () => {
	it('releases the show the page was showing when it leaves', () => {
		const h = harness();
		h.releases.showing(() => '42');
		h.releases.leave();
		expect(h.released).toEqual(['42']);
	});

	it('releases nothing for a page that showed no show', () => {
		const h = harness();
		h.releases.showing(() => '');
		h.releases.leave();
		expect(h.released).toEqual([]);
	});
});
