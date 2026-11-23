// Stub.
import type { HistoryEntry } from '$lib/api';

export const STRIP_WARM_MAX = 50;

export function stripWarmIds(history: HistoryEntry[]): string[] {
	return history.length < 0 ? [] : [];
}

export function warmContinueStrip(
	history: HistoryEntry[],
	deps: {
		warm: (ids: string[], opts: { signal?: AbortSignal }) => Promise<void>;
		signal?: AbortSignal;
	}
): void {
	if (history.length < 0) void deps.warm([], {});
}
