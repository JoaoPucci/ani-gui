// The backend's watch-order answers, as the acceptance scenarios serve
// them: every decision the watch-order surfaces draw is the backend's
// (`POST /api/watch-order/:id/<page>`), and the frontend's CI runs no
// Rust, so each scenario's answer is recorded by the backend from the
// scenario's own inputs and held there (tests/fixtures/watch-order/
// scenarios/<suite>.json, checked by backend/src/watch_order/
// scenarios_test.rs). A scenario serves the answer recorded for the
// inputs it describes — the franchise the walk answers, the history
// rows the resume lookup names, the order setting, the episode — and
// asserts on what the page draws from it. The decisions themselves are
// asserted in Rust, by name.
//
// A scenario whose inputs have no recorded answer fails, naming them.
// To record new inputs:
//
//   1. `WATCH_ORDER_RECORD=1 pnpm run test:acceptance <files>`: each
//      suite file is rewritten with the inputs its scenarios sent (the
//      ones it no longer sends dropped), new ones with no answer yet,
//      and the scenarios needing them fail;
//   2. in backend/, `cargo test --lib watch_order::scenarios -- --ignored`
//      fills in the answers;
//   3. run the scenarios again.
//
// Home's rail (`POST /api/watch-order/rail`) is pure, and read again as
// often as Home learns something, so it is answered by a fake
// (src/test/watch-order-rail-fake.ts) rather than a record; the
// requests the scenarios sent are recorded all the same, so the
// backend checks its answers and the fake is held to them.

import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { resolve as resolvePath } from 'node:path';
import { expect } from 'vitest';
import { http, HttpResponse } from 'msw';

import type {
	Franchise,
	FranchiseView,
	HandoffAnswer,
	RailRequest,
	RailView,
	ShowView,
	UpNextView,
	WatchOrder
} from '../../src/lib/api';
import { errorAnswer } from '../answers';
import { railView } from '../../src/test/watch-order-rail-fake';
import { API_BASE } from './setup';

/** The day every recorded answer is read on: what "not aired yet" is judged against. */
export const SCENARIO_TODAY = '2026-11-24';

export type WatchOrderRoute = 'show' | 'franchise' | 'up-next' | 'handoff';

/** What one of the routes answers. */
type RouteAnswer = ShowView | FranchiseView | UpNextView | HandoffAnswer;

/** What the player's detail says about the entry, which decides its finale. */
export interface OwnReading {
	status: string | null;
	episodes: number | null;
}

/** One read's inputs, as the backend reads them. */
export interface ScenarioInputs {
	route: WatchOrderRoute;
	id: string;
	order: WatchOrder;
	/** The franchise the walk answers for `id`. */
	franchise: Franchise;
	/**
	 * What the resume lookup answers: each Kitsu id a history row
	 * resumes, with that row's episode; null when the history cannot be
	 * read.
	 */
	resumes: Record<string, string> | null;
	/** `up-next`: the episode playing. */
	episode?: number;
	/** `up-next`: the player's own reading of the entry, where it has one. */
	own?: OwnReading | null;
}

interface RecordedCase {
	name: string;
	route: WatchOrderRoute;
	id: string;
	order: WatchOrder;
	franchise: string;
	resumes: Record<string, string> | null;
	episode?: number;
	own?: OwnReading;
	answer: unknown;
}

interface RecordedRail {
	name: string;
	request: RailRequest;
	answer: RailView | null;
}

export interface Suite {
	today: string;
	franchises: Record<string, Franchise>;
	cases: RecordedCase[];
	rails: RecordedRail[];
}

const RECORDING = process.env.WATCH_ORDER_RECORD === '1';
const DIR = resolvePath(__dirname, '../../../tests/fixtures/watch-order/scenarios');

/** `value` as JSON with every object's keys sorted, `undefined` left out. */
export function canonical(value: unknown): string {
	return JSON.stringify(value, (_key, v: unknown) =>
		v && typeof v === 'object' && !Array.isArray(v)
			? Object.fromEntries(Object.entries(v).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)))
			: v
	);
}

const franchiseKey = (f: Franchise) =>
	createHash('sha1').update(canonical(f)).digest('hex').slice(0, 12);

/** A recorded case's inputs, as the lookup compares them. */
function caseKey(c: Omit<RecordedCase, 'name' | 'answer'>): string {
	const { route, id, order, franchise, resumes, episode, own } = c;
	return canonical({ route, id, order, franchise, resumes, episode, own });
}

export function readSuite(suite: string): Suite {
	const path = resolvePath(DIR, `${suite}.json`);
	if (!existsSync(path)) return { today: SCENARIO_TODAY, franchises: {}, cases: [], rails: [] };
	return JSON.parse(readFileSync(path, 'utf8')) as Suite;
}

/** The suite files' names. */
export function suiteNames(): string[] {
	if (!existsSync(DIR)) return [];
	return readdirSync(DIR)
		.filter((f) => f.endsWith('.json'))
		.map((f) => f.slice(0, -5));
}

/** One suite's record as a scenario run reads and, recording, rewrites it. */
class SuiteRecord {
	private readonly file: Suite;
	private readonly byKey = new Map<string, RecordedCase>();
	private readonly railsByKey = new Map<string, RecordedRail>();
	// What this run asked for, in order: a recording keeps these only.
	private readonly used: RecordedCase[] = [];
	private readonly usedKeys = new Set<string>();
	private readonly usedRails: RecordedRail[] = [];
	private readonly usedFranchises: Record<string, Franchise> = {};

	constructor(private readonly suite: string) {
		this.file = readSuite(suite);
		for (const c of this.file.cases) this.byKey.set(caseKey(c), c);
		for (const r of this.file.rails) this.railsByKey.set(canonical(r.request), r);
	}

	answer(inputs: ScenarioInputs): unknown {
		const key = franchiseKey(inputs.franchise);
		const shape = {
			route: inputs.route,
			id: inputs.id,
			order: inputs.order,
			franchise: key,
			resumes: inputs.resumes,
			...(inputs.route === 'up-next'
				? { episode: inputs.episode, own: inputs.own ?? undefined }
				: {})
		};
		const k = caseKey(shape);
		const found = this.byKey.get(k);
		if (RECORDING && !this.usedKeys.has(k)) {
			this.usedKeys.add(k);
			this.usedFranchises[key] = inputs.franchise;
			this.used.push(found ?? { name: this.nameFor(shape), ...shape, answer: null });
			this.write();
		}
		if (found?.answer == null) {
			if (RECORDING) return undefined;
			throw new Error(
				`no recorded watch-order answer in scenarios/${this.suite}.json for ${canonical({
					...shape,
					franchise: inputs.franchise
				})}\n(record it: see tests/acceptance/watch-order-answers.ts)`
			);
		}
		return found.answer;
	}

	rail(request: RailRequest): RailView {
		const answer = railView(request);
		if (RECORDING) {
			const k = canonical(request);
			if (!this.usedRails.some((r) => canonical(r.request) === k)) {
				const found = this.railsByKey.get(k);
				this.usedRails.push(found ?? { name: currentName(), request, answer: null });
				this.write();
			}
		}
		return answer;
	}

	private nameFor(shape: Omit<RecordedCase, 'name' | 'answer'>): string {
		const base = `${currentName()} · ${shape.route} ${shape.id}${
			shape.episode !== undefined ? ` ep ${shape.episode}` : ''
		}`;
		let name = base;
		for (let n = 2; this.used.some((c) => c.name === name); n++) name = `${base} #${n}`;
		return name;
	}

	private write(): void {
		mkdirSync(DIR, { recursive: true });
		const out: Suite = {
			today: SCENARIO_TODAY,
			franchises: this.usedFranchises,
			cases: this.used,
			rails: this.usedRails
		};
		writeFileSync(resolvePath(DIR, `${this.suite}.json`), `${JSON.stringify(out, null, 2)}\n`);
	}
}

function currentName(): string {
	return expect.getState().currentTestName ?? '(outside a scenario)';
}

const records = new Map<string, SuiteRecord>();
function recordOf(suite: string): SuiteRecord {
	let r = records.get(suite);
	if (!r) records.set(suite, (r = new SuiteRecord(suite)));
	return r;
}

/** The backend's recorded answer for `inputs`, in suite `suite`. */
export function recordedAnswer<T>(suite: string, inputs: ScenarioInputs): T {
	return recordOf(suite).answer(inputs) as T;
}

/**
 * The recorded answer for `inputs` as the route serves it: a 503 when
 * the scenario is being recorded and the answer is not in yet.
 */
export function answerResponse(suite: string, inputs: ScenarioInputs): Response {
	const answer = recordOf(suite).answer(inputs);
	if (answer === undefined) return errorAnswer(503);
	return HttpResponse.json<RouteAnswer>(answer as RouteAnswer);
}

/** The rail route, answered as the backend does (`railView`), recorded under `suite`. */
export function railHandler(suite: string) {
	return http.post(`${API_BASE}/api/watch-order/rail`, async ({ request }) =>
		HttpResponse.json<RailView>(recordOf(suite).rail((await request.json()) as RailRequest))
	);
}
