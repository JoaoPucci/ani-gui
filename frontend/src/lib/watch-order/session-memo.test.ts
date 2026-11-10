import { afterEach, describe, expect, it } from 'vitest';
import {
	MEMO_LIMIT,
	forgetAnswers,
	rememberedAnswer,
	settledAnswer,
	visitAnswer
} from './session-memo';
import type { WatchOrderData } from './loader';
import type { FranchiseData } from './types';

const franchise = (anchor: string): FranchiseData => ({
	anchor,
	entries: [],
	links: [],
	versions: [],
	films: [],
	sides: [],
	recaps: [],
	parents: {},
	complete: true
});
const answer = (id: string, ep = '1'): WatchOrderData => ({
	franchise: franchise(id),
	rows: [{ kitsu_id: id, ep_no: ep }]
});
const failed: WatchOrderData = { franchise: null, rows: [] };

afterEach(forgetAnswers);

describe('the session memo of watch-order answers', () => {
	it('remembers the last answer per show', () => {
		expect(rememberedAnswer('1')).toBeNull();
		expect(settledAnswer('1', answer('1'))).toEqual(answer('1'));
		expect(rememberedAnswer('1')).toEqual(answer('1'));
		expect(settledAnswer('1', answer('1', '4'))).toEqual(answer('1', '4'));
		expect(rememberedAnswer('1')).toEqual(answer('1', '4'));
		expect(rememberedAnswer('2')).toBeNull();
	});

	it('keeps the earlier answer when a later read fails', () => {
		settledAnswer('1', answer('1'));
		expect(settledAnswer('1', failed)).toEqual(answer('1'));
		expect(rememberedAnswer('1')).toEqual(answer('1'));
	});

	it('remembers nothing of a failed read', () => {
		expect(settledAnswer('1', failed)).toEqual(failed);
		expect(rememberedAnswer('1')).toBeNull();
	});

	it('forgets the least recently used show past its limit', () => {
		for (let i = 0; i < MEMO_LIMIT; i++) settledAnswer(String(i), answer(String(i)));
		// Show 0 is read again, so show 1 is now the oldest.
		expect(rememberedAnswer('0')).not.toBeNull();
		settledAnswer('new', answer('new'));
		expect(rememberedAnswer('0')).not.toBeNull();
		expect(rememberedAnswer('1')).toBeNull();
		expect(rememberedAnswer('new')).not.toBeNull();
	});

	it('forgets everything on request', () => {
		settledAnswer('1', answer('1'));
		forgetAnswers();
		expect(rememberedAnswer('1')).toBeNull();
	});
});

describe("a show page visit's answer", () => {
	it('draws and remembers the answer of a visit still on screen', () => {
		const visit = new AbortController();
		expect(visitAnswer('1', answer('1'), visit.signal)).toEqual(answer('1'));
		expect(rememberedAnswer('1')).toEqual(answer('1'));
	});

	it('keeps the earlier answer for a visit whose read failed', () => {
		settledAnswer('1', answer('1'));
		expect(visitAnswer('1', failed, new AbortController().signal)).toEqual(answer('1'));
	});

	it('draws nothing for a visit that ended, and leaves the memo as it was', () => {
		// The viewer left, or the page read the show again: what the
		// cancelled read answered is no answer at all.
		settledAnswer('1', answer('1', '3'));
		const visit = new AbortController();
		visit.abort();
		expect(visitAnswer('1', failed, visit.signal)).toBeNull();
		expect(visitAnswer('1', answer('1', '9'), visit.signal)).toBeNull();
		expect(rememberedAnswer('1')).toEqual(answer('1', '3'));
		expect(visitAnswer('2', answer('2'), visit.signal)).toBeNull();
		expect(rememberedAnswer('2')).toBeNull();
	});
});
