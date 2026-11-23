// The renderer's watch-order modules against the backend's parity
// record (tests/fixtures/watch-order/parity.json, written by
// backend/src/watch_order/parity_test.rs): every decision the backend's
// port makes, over the fixture franchises, the walk's served corpus and
// a fixed set of generated payloads. While both exist, this holds the
// two to the same answers.

import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve as resolvePath } from 'node:path';
import { FIXTURE_RUNS } from './fixtures/runs';
import { franchisePage } from './franchise-page';
import { handoffAsk, handoffFrom } from './handoff';
import { franchiseLinkOnly, watchOrderPage } from './page-model';
import { playerUpNext } from './chain-end';
import type { HistoryRow } from './progress';
import type { WatchOrder } from './by-start';
import type { FranchiseData } from './types';

const ROOT = resolvePath(__dirname, '../../../../tests/fixtures');

interface Recorded {
	today: string;
	cases: {
		input: {
			name: string;
			source: { fixture?: string; anchor?: string; served?: string; payload?: FranchiseData };
			rows: HistoryRow[];
			played: string[];
		};
		answer: { link: unknown; orders: Record<WatchOrder, Record<string, unknown>> };
	}[];
	asks: {
		input: {
			matched: { id: string; status: string | null; episode_count: number | null } | null;
			ep_no: string;
			playable_count: number | null;
			left_part_way: boolean;
		};
		answer: string | null;
	}[];
}

const record = JSON.parse(
	readFileSync(resolvePath(ROOT, 'watch-order/parity.json'), 'utf8')
) as Recorded;

/** A fixture run as the backend's parity record serves it for `anchor`. */
function fixture(key: string, anchor: string): FranchiseData {
	const r = FIXTURE_RUNS.find((x) => x.key === key)!;
	return {
		anchor,
		name: `The ${key} franchise`,
		name_from: r.nameId,
		entries: r.entries,
		links: r.links,
		versions: r.versions,
		films: r.films,
		sides: r.sides,
		recaps: r.recaps,
		parents: r.parents[anchor] ? { [anchor]: r.parents[anchor] } : {},
		complete: true
	};
}

function franchiseOf(source: Recorded['cases'][number]['input']['source']): FranchiseData {
	if (source.fixture) return fixture(source.fixture, source.anchor!);
	if (source.served)
		return JSON.parse(
			readFileSync(resolvePath(ROOT, `franchise/served-${source.served}.json`), 'utf8')
		) as FranchiseData;
	return source.payload!;
}

const snake = (key: string) => key.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);

/** `v` in the record's shape: snake_case keys, sets as sorted lists,
 *  nothing undefined, and every entry as its id. */
function wire(v: unknown): unknown {
	if (v === undefined || v === null) return null;
	if (v instanceof Set) return [...v].map(wire).sort();
	if (Array.isArray(v)) return v.map(wire);
	if (typeof v !== 'object') return v;
	const o = v as Record<string, unknown>;
	if ('id' in o && 'title' in o && 'subtype' in o) return o.id;
	return Object.fromEntries(Object.entries(o).map(([k, x]) => [snake(k), wire(x)]));
}

function franchiseAnswer(f: FranchiseData, rows: HistoryRow[], order: WatchOrder): unknown {
	const page = franchisePage({ franchise: f, rows }, f.anchor, order);
	if (!page) return null;
	// The backend's page lists the rails on its run alone, and names the
	// title it was opened from (the trail's link back).
	const rest: Partial<typeof page> = { ...page };
	for (const k of ['versions', 'films', 'sides', 'recaps'] as const) delete rest[k];
	const r = page.run;
	const from = [...r.entries, ...r.versions, ...r.films, ...r.sides, ...r.recaps].find(
		(e) => e.id === f.anchor
	);
	return wire({ ...rest, openedFrom: from ?? null });
}

describe('the backend port decides as these modules do', () => {
	it('on every recorded case, in both orders', () => {
		expect(record.cases.length).toBeGreaterThan(400);
		for (const { input, answer } of record.cases) {
			const f = franchiseOf(input.source);
			const id = f.anchor;
			const data = { franchise: f, rows: input.rows };
			expect(wire(franchiseLinkOnly(data, id)), `${input.name} link`).toEqual(answer.link);
			for (const order of ['linked', 'released'] as const) {
				const want = answer.orders[order];
				const at = `${input.name} ${order}`;
				expect(wire(watchOrderPage(data, id, order)), `${at} page`).toEqual(want.page);
				expect(franchiseAnswer(f, input.rows, order), `${at} franchise`).toEqual(want.franchise);
				const episode = want.episode as number;
				expect(
					wire(playerUpNext(data, id, order, record.today, episode, null)),
					`${at} up next`
				).toEqual(want.up_next);
				expect(
					wire(handoffFrom(f, id, order, record.today, new Set(input.played))),
					`${at} hand-off`
				).toEqual(want.handoff);
			}
		}
	});

	it('on every recorded hand-off ask', () => {
		for (const { input, answer } of record.asks) {
			const got = handoffAsk(input.matched, input.ep_no, input.playable_count, () =>
				Boolean(input.left_part_way)
			);
			expect(got, JSON.stringify(input)).toBe(answer);
		}
	});
});
