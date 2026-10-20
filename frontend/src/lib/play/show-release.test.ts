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

describe('createShowReleases, once the route has moved on', () => {
	// A page's cleanup runs after the route already names where the
	// viewer went: no show for home, another show for that show's page.
	it('releases the show the page was showing, not the one the route names now', () => {
		const h = harness();
		let route = '42';
		h.releases.showing(() => route);
		route = '77';
		h.releases.leave();
		expect(h.released).toEqual(['42']);
	});

	it('releases the show the page was showing when the route names none any more', () => {
		const h = harness();
		let route = '42';
		h.releases.showing(() => route);
		route = '';
		h.releases.leave();
		expect(h.released).toEqual(['42']);
	});

	// The detail and player routes keep their component across a move
	// to another show, and a play started for the first show still
	// answers there, so its release waits for the page to go.
	it('releases every show the page showed, once each', () => {
		const h = harness();
		h.releases.showing(() => '42');
		h.releases.showing(() => '77');
		h.releases.showing(() => '42');
		expect(h.released).toEqual([]);
		h.releases.leave();
		expect([...h.released].sort()).toEqual(['42', '77']);
	});
});
