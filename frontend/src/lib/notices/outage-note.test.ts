import { describe, expect, it } from 'vitest';
import { createOutageNote } from './outage-note';
import type { Published } from './published';
import { outageChanges, type ProviderHealth } from './sources';

const DOWN: ProviderHealth = {
	provider: 'hianime',
	state: 'not_answering',
	since_ms: 1,
	failures: 3,
	last_failure_ms: 2
};
const ABOUT_SOURCE: Published = {
	id: 'src',
	title: 'Down for maintenance',
	body: null,
	link: null,
	priority: 'warning',
	aboutSource: true
};
const NOTE = { noteTitle: 'Down for maintenance', noteBody: null, noteLink: null };
const raisedData = (changes: ReturnType<typeof outageChanges>) =>
	changes.map((c) => (c.op === 'raise' ? c.input.data : null));

describe('the published note on the outage row, whichever answer comes first', () => {
	it('joins the row the health already raised once the published answer arrives', () => {
		const join = createOutageNote();
		expect(raisedData(join.health([DOWN]))).toEqual([{ since: 1, failures: 3, last: 2 }]);
		expect(raisedData(join.published([ABOUT_SOURCE]))).toEqual([
			{ since: 1, failures: 3, last: 2, ...NOTE }
		]);
	});

	it('changes nothing when the published answer arrives before any health', () => {
		const join = createOutageNote();
		expect(join.published([ABOUT_SOURCE])).toEqual([]);
		expect(raisedData(join.health([DOWN]))).toEqual([{ since: 1, failures: 3, last: 2, ...NOTE }]);
	});

	it('adds no note when nothing published is about the source', () => {
		const join = createOutageNote();
		join.health([DOWN]);
		expect(join.published([{ ...ABOUT_SOURCE, aboutSource: false }])).toEqual(
			outageChanges([DOWN])
		);
		expect(join.health([DOWN])).toEqual(outageChanges([DOWN]));
	});

	it('joins the note to the latest health answer, not the first', () => {
		const answering: ProviderHealth = { provider: 'hianime', state: 'answering' };
		const join = createOutageNote();
		join.health([DOWN]);
		join.health([answering]);
		expect(join.published([ABOUT_SOURCE])).toEqual(outageChanges([answering]));
	});
});
