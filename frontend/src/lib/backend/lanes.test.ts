import { describe, expect, it } from 'vitest';
import { laneBase, LANES } from './lanes';

const API = 'http://127.0.0.1:1';

describe('laneBase', () => {
	it("answers a lane's own origin when the backend announced one", () => {
		const lanes = { probe: 'http://127.0.0.1:3', stream: 'http://127.0.0.1:2/' };
		expect(laneBase(lanes, 'probe', API)).toBe('http://127.0.0.1:3');
		// A trailing slash would double up against every path.
		expect(laneBase(lanes, 'stream', API)).toBe('http://127.0.0.1:2');
	});

	it("falls back to the API's origin for a lane nobody announced", () => {
		// Browser-only dev, or a window opened before the backend had
		// lanes: every lane serves every route, so the API's origin
		// still answers — it just shares one connection budget.
		expect(laneBase(undefined, 'image', API)).toBe(API);
		expect(laneBase({}, 'image', API)).toBe(API);
		expect(laneBase({ image: '' }, 'image', API)).toBe(API);
	});

	it('the API lane is always the API origin', () => {
		expect(laneBase({ api: 'http://127.0.0.1:9' }, 'api', `${API}/`)).toBe(API);
	});

	it('names every lane the backend announces', () => {
		expect([...LANES].sort()).toEqual(
			['api', 'download', 'image', 'probe', 'resolve', 'stream'].sort()
		);
	});
});
