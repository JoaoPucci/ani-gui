/**
 * The Diagnostics page's actions on what it loads: Refresh (app info and
 * the history list), Clear history, and Clear metadata cache. The page
 * is a thin adapter: it renders the state this hands it and wires the
 * buttons to the actions.
 *
 * The rules:
 *
 *   - One action at a time. Every action holds `busy` while it is out,
 *     and an action asked for while another is out starts nothing. A
 *     list read before a clear and answered after it would otherwise
 *     bring the rows back.
 *   - A message is about the last action. A refresh takes down every
 *     earlier failure of the history section, the clear's included, and
 *     shows what the load finds. A clear takes down its own earlier
 *     failure as it starts; when it lands the list is known empty, so a
 *     load's earlier failure goes too. A clear that fails leaves an
 *     earlier load's failure in place, beside its own.
 *   - The metadata cache clear keeps its own outcome, taken down when
 *     it starts again.
 */

import type { AppInfo, HistoryEntry } from '$lib/api';

export interface DiagnosticsState {
	info: AppInfo | null;
	infoError: string | null;
	/** Null until a load has read the list. */
	history: HistoryEntry[] | null;
	/** The last load's failure. */
	historyError: string | null;
	/** The last clear's failure. */
	historyClearError: string | null;
	/** Whether the last metadata cache clear landed. */
	cacheCleared: boolean;
	cacheError: string | null;
	/** Whether an action is out. */
	busy: boolean;
}

export interface DiagnosticsDeps {
	appInfo(): Promise<AppInfo>;
	historyList(): Promise<HistoryEntry[]>;
	historyClear(): Promise<void>;
	metaCacheClear(): Promise<void>;
	/** The words a failure is shown with. */
	describe(error: unknown): string;
	/** Hands the page each new state. */
	changed(state: DiagnosticsState): void;
}

export interface DiagnosticsActions {
	readonly state: DiagnosticsState;
	refresh(): Promise<void>;
	clearHistory(): Promise<void>;
	clearCache(): Promise<void>;
}

export const INITIAL_DIAGNOSTICS: DiagnosticsState = {
	info: null,
	infoError: null,
	history: null,
	historyError: null,
	historyClearError: null,
	cacheCleared: false,
	cacheError: null,
	busy: false
};

export function createDiagnosticsActions(deps: DiagnosticsDeps): DiagnosticsActions {
	let state: DiagnosticsState = { ...INITIAL_DIAGNOSTICS };
	const set = (patch: Partial<DiagnosticsState>) => {
		state = { ...state, ...patch };
		deps.changed(state);
	};

	/** Runs `body` holding `busy`, unless another action is out. */
	async function exclusive(start: Partial<DiagnosticsState>, body: () => Promise<void>) {
		if (state.busy) return;
		set({ ...start, busy: true });
		try {
			await body();
		} finally {
			set({ busy: false });
		}
	}

	return {
		get state() {
			return state;
		},
		refresh: () =>
			exclusive({ infoError: null, historyError: null, historyClearError: null }, async () => {
				try {
					set({ info: await deps.appInfo() });
				} catch (e) {
					set({ infoError: deps.describe(e) });
				}
				try {
					set({ history: await deps.historyList() });
				} catch (e) {
					set({ historyError: deps.describe(e) });
				}
			}),
		clearHistory: () =>
			exclusive({ historyClearError: null }, async () => {
				try {
					await deps.historyClear();
					set({ history: [], historyError: null });
				} catch (e) {
					set({ historyClearError: deps.describe(e) });
				}
			}),
		clearCache: () =>
			exclusive({ cacheCleared: false, cacheError: null }, async () => {
				try {
					await deps.metaCacheClear();
					set({ cacheCleared: true });
				} catch (e) {
					set({ cacheError: deps.describe(e) });
				}
			})
	};
}
