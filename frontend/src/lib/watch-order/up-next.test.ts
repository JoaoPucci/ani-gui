import { describe, expect, it } from 'vitest';
import { FIXTURE_RUNS, type FixtureRun } from './fixtures/runs';
import { upNextFor } from './up-next';

const TODAY = '2026-11-02';
const run = (key: string): FixtureRun => FIXTURE_RUNS.find((r) => r.key === key)!;

describe('upNextFor', () => {
	it('answers the end card in each of its four cases', () => {
		expect(upNextFor(run('bleach'), '43078', 'linked', TODAY)).toMatchObject({
			kind: 'one',
			entry: { id: '46903' }
		});
		expect(upNextFor(run('monogatari'), '48434', 'linked', TODAY)).toMatchObject({
			kind: 'unaired',
			entry: { id: '50013' }
		});
		expect(upNextFor(run('fate'), '7658', 'linked', TODAY).kind).toBe('choices');
		expect(upNextFor(run('bleach'), '49444', 'linked', TODAY).kind).toBe('none');
	});
});
