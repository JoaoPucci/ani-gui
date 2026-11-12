import { describe, expect, test } from 'vitest';
import fc from 'fast-check';
import type { AppInfo, HistoryEntry } from '$lib/api';
import {
	createDiagnosticsActions,
	INITIAL_DIAGNOSTICS,
	type DiagnosticsDeps,
	type DiagnosticsState
} from './history-actions';

const INFO = { version: '1.0.0' } as AppInfo;
const ROW = { id: 'allanime-1', ep_no: '3', title: 'Cowboy Bebop' } as HistoryEntry;

/** A backend whose answers the test sets per call, and which can hold
 *  a call until released. */
function backend() {
	const answers = { info: true, list: true, clear: true, cache: true };
	const calls: string[] = [];
	let hold: Promise<void> | null = null;
	const states: DiagnosticsState[] = [];
	const answer = async <T>(name: string, ok: boolean, value: T): Promise<T> => {
		calls.push(name);
		if (hold) await hold;
		if (!ok) throw new Error(`${name} down`);
		return value;
	};
	const deps: DiagnosticsDeps = {
		appInfo: () => answer('info', answers.info, INFO),
		historyList: () => answer('list', answers.list, [ROW]),
		historyClear: () => answer('clear', answers.clear, undefined),
		metaCacheClear: () => answer('cache', answers.cache, undefined),
		describe: (e) => (e as Error).message,
		changed: (s) => states.push(s)
	};
	return {
		answers,
		calls,
		states,
		deps,
		holdNext() {
			let release: () => void = () => {};
			hold = new Promise<void>((r) => (release = r));
			return () => {
				hold = null;
				release();
			};
		}
	};
}

describe('createDiagnosticsActions', () => {
	test('starts with nothing loaded and nothing out', () => {
		const b = backend();
		expect(createDiagnosticsActions(b.deps).state).toEqual(INITIAL_DIAGNOSTICS);
	});

	test('a refresh loads app info and the list, holding busy while it reads', async () => {
		const b = backend();
		const a = createDiagnosticsActions(b.deps);
		const release = b.holdNext();
		const done = a.refresh();
		expect(a.state.busy).toBe(true);
		release();
		await done;
		expect(a.state).toMatchObject({ info: INFO, history: [ROW], busy: false });
		expect(b.states[b.states.length - 1]).toBe(a.state);
	});

	test('a refresh whose reads fail shows each failure, and keeps what it had', async () => {
		const b = backend();
		const a = createDiagnosticsActions(b.deps);
		await a.refresh();
		b.answers.info = false;
		b.answers.list = false;
		await a.refresh();
		expect(a.state).toMatchObject({
			info: INFO,
			infoError: 'info down',
			history: [ROW],
			historyError: 'list down',
			busy: false
		});
	});

	test('a refresh takes down an earlier clear’s failure and an earlier load’s', async () => {
		const b = backend();
		const a = createDiagnosticsActions(b.deps);
		b.answers.list = false;
		await a.refresh();
		b.answers.clear = false;
		await a.clearHistory();
		expect(a.state.historyClearError).toBe('clear down');
		b.answers.list = true;
		await a.refresh();
		expect(a.state).toMatchObject({ history: [ROW], historyError: null, historyClearError: null });
	});

	test('a refresh that fails shows the load’s failure in place of the clear’s', async () => {
		const b = backend();
		const a = createDiagnosticsActions(b.deps);
		await a.refresh();
		b.answers.clear = false;
		await a.clearHistory();
		b.answers.list = false;
		await a.refresh();
		expect(a.state).toMatchObject({ historyError: 'list down', historyClearError: null });
	});

	test('a clear that lands empties the list and takes a load’s failure down', async () => {
		const b = backend();
		const a = createDiagnosticsActions(b.deps);
		b.answers.list = false;
		await a.refresh();
		await a.clearHistory();
		expect(a.state).toMatchObject({ history: [], historyError: null, historyClearError: null });
	});

	test('a clear that fails keeps the list, and a load’s failure while there is no list', async () => {
		const b = backend();
		const a = createDiagnosticsActions(b.deps);
		await a.refresh();
		b.answers.clear = false;
		await a.clearHistory();
		expect(a.state).toMatchObject({ history: [ROW], historyClearError: 'clear down' });

		const c = backend();
		const fresh = createDiagnosticsActions(c.deps);
		c.answers.list = false;
		await fresh.refresh();
		c.answers.clear = false;
		await fresh.clearHistory();
		expect(fresh.state).toMatchObject({
			history: null,
			historyError: 'list down',
			historyClearError: 'clear down'
		});
	});

	test('a clear takes its own earlier failure down as it starts', async () => {
		const b = backend();
		const a = createDiagnosticsActions(b.deps);
		await a.refresh();
		b.answers.clear = false;
		await a.clearHistory();
		const release = b.holdNext();
		const done = a.clearHistory();
		expect(a.state).toMatchObject({ historyClearError: null, busy: true });
		release();
		await done;
	});

	test('the cache clear keeps its own outcome, taken down when it starts again', async () => {
		const b = backend();
		const a = createDiagnosticsActions(b.deps);
		await a.clearCache();
		expect(a.state).toMatchObject({ cacheCleared: true, cacheError: null });
		b.answers.cache = false;
		const release = b.holdNext();
		const done = a.clearCache();
		expect(a.state).toMatchObject({ cacheCleared: false, cacheError: null });
		release();
		await done;
		expect(a.state).toMatchObject({ cacheCleared: false, cacheError: 'cache down' });
	});

	test('an action asked for while another is out starts nothing', async () => {
		const b = backend();
		const a = createDiagnosticsActions(b.deps);
		const release = b.holdNext();
		const refreshing = a.refresh();
		await a.clearHistory();
		await a.clearCache();
		await a.refresh();
		release();
		await refreshing;
		expect(b.calls).toEqual(['info', 'list']);
		expect(a.state.history).toEqual([ROW]);
	});

	test('busy is released when an action fails', async () => {
		const b = backend();
		const a = createDiagnosticsActions(b.deps);
		b.answers.clear = false;
		await a.clearHistory();
		expect(a.state.busy).toBe(false);
	});
});

type Step =
	| { kind: 'refresh'; info: boolean; list: boolean }
	| { kind: 'clear'; ok: boolean }
	| { kind: 'cache'; ok: boolean };

const step: fc.Arbitrary<Step> = fc.oneof(
	fc.record({ kind: fc.constant('refresh' as const), info: fc.boolean(), list: fc.boolean() }),
	fc.record({ kind: fc.constant('clear' as const), ok: fc.boolean() }),
	fc.record({ kind: fc.constant('cache' as const), ok: fc.boolean() })
);

describe('createDiagnosticsActions — property', () => {
	test('over any run of refreshes, clears and cache clears, the page shows what the last action left', async () => {
		await fc.assert(
			fc.asyncProperty(fc.array(step, { maxLength: 12 }), async (steps) => {
				const b = backend();
				const a = createDiagnosticsActions(b.deps);
				// A model of what should be on show.
				let info: AppInfo | null = null;
				let infoError: string | null = null;
				let history: HistoryEntry[] | null = null;
				let historyError: string | null = null;
				let clearError: string | null = null;
				let cacheCleared = false;
				let cacheError: string | null = null;
				for (const s of steps) {
					if (s.kind === 'refresh') {
						b.answers.info = s.info;
						b.answers.list = s.list;
						await a.refresh();
						clearError = null;
						infoError = s.info ? null : 'info down';
						if (s.info) info = INFO;
						historyError = s.list ? null : 'list down';
						if (s.list) history = [ROW];
					} else if (s.kind === 'clear') {
						b.answers.clear = s.ok;
						await a.clearHistory();
						if (s.ok) {
							history = [];
							historyError = null;
							clearError = null;
						} else clearError = 'clear down';
					} else {
						b.answers.cache = s.ok;
						await a.clearCache();
						cacheCleared = s.ok;
						cacheError = s.ok ? null : 'cache down';
					}
					expect(a.state).toMatchObject({
						info,
						infoError,
						history,
						historyError,
						historyClearError: clearError,
						cacheCleared,
						cacheError,
						busy: false
					});
				}
			})
		);
	});

	test('however the actions overlap, only the first one out runs', async () => {
		await fc.assert(
			fc.asyncProperty(
				fc.array(fc.constantFrom('refresh', 'clear', 'cache'), { minLength: 1, maxLength: 6 }),
				async (kinds) => {
					const b = backend();
					const a = createDiagnosticsActions(b.deps);
					const release = b.holdNext();
					const run = (k: string) =>
						k === 'refresh' ? a.refresh() : k === 'clear' ? a.clearHistory() : a.clearCache();
					const all = kinds.map(run);
					release();
					await Promise.all(all);
					const first = kinds[0];
					const expected = first === 'refresh' ? ['info', 'list'] : [first];
					expect(b.calls).toEqual(expected);
					expect(a.state.busy).toBe(false);
				}
			)
		);
	});
});
