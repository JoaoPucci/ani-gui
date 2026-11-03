/**
 * What raises a notice: the update check, the download list, and the
 * source's health as the backend reads it from the app's own traffic.
 * Each is turned into keyed changes here; none makes a request.
 */
import type { NoticeData, RaiseInput } from './model';

export type SourceChange =
	| { op: 'raise'; input: RaiseInput }
	| { op: 'resolve'; key: string; at: number; data: NoticeData }
	| { op: 'withdraw'; key: string };

export const UPDATE_KEY = 'update';

/** The update row: raised while a newer release exists, withdrawn
 *  once it does not (the update was installed). */
export function updateChange(tag: string | null, current: string): SourceChange {
	if (tag === null) return { op: 'withdraw', key: UPDATE_KEY };
	return { op: 'raise', input: { key: UPDATE_KEY, kind: 'update', data: { tag, current } } };
}

/** One provider's answer from `/api/source-health`. */
export type ProviderHealth =
	| { provider: string; state: 'answering' }
	| {
			provider: string;
			state: 'not_answering';
			since_ms: number;
			failures: number;
			last_failure_ms: number;
	  }
	| { provider: string; state: 'answering_again'; since_ms: number; recovered_ms: number };

const num = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v) && v >= 0;

/** The providers of a `/api/source-health` answer; anything that is
 *  not one is left out. */
export function parseHealth(body: unknown): ProviderHealth[] {
	const list = (body as { providers?: unknown } | null)?.providers;
	if (!Array.isArray(list)) return [];
	return list.flatMap((p): ProviderHealth[] => {
		const o = (typeof p === 'object' && p !== null ? p : {}) as Record<string, unknown>;
		if (typeof o.provider !== 'string' || !/^[a-z0-9-]{1,32}$/.test(o.provider)) return [];
		const provider = o.provider;
		if (o.state === 'answering') return [{ provider, state: 'answering' }];
		if (o.state === 'not_answering' && num(o.since_ms) && num(o.failures) && num(o.last_failure_ms))
			return [
				{
					provider,
					state: 'not_answering',
					since_ms: o.since_ms,
					failures: o.failures,
					last_failure_ms: o.last_failure_ms
				}
			];
		if (o.state === 'answering_again' && num(o.since_ms) && num(o.recovered_ms))
			return [
				{
					provider,
					state: 'answering_again',
					since_ms: o.since_ms,
					recovered_ms: o.recovered_ms
				}
			];
		return [];
	});
}

export const outageKey = (provider: string) => `outage:${provider}`;

/** The outage rows the providers' health asks for: raised while a
 *  provider is not answering, resolved once it answers again. */
export function outageChanges(health: readonly ProviderHealth[]): SourceChange[] {
	return health.flatMap((h): SourceChange[] => {
		const key = outageKey(h.provider);
		if (h.state === 'not_answering')
			return [
				{
					op: 'raise',
					input: {
						key,
						kind: 'outage',
						count: 0,
						data: { since: h.since_ms, failures: h.failures, last: h.last_failure_ms }
					}
				}
			];
		if (h.state === 'answering_again')
			return [{ op: 'resolve', key, at: h.recovered_ms, data: { since: h.since_ms } }];
		return [];
	});
}

/** The part of a download the center reads. */
export interface DownloadOutcome {
	id: string;
	title: string;
	episode: string;
	status: string;
	destDir: string;
	error: string | null;
	startedAt: number;
	rangeTotal: number | null;
}

/** Rows for downloads that finished or failed since `seen` was last
 *  updated; the ids it reports are added to `seen`. Finished episodes
 *  of one show share a row and add to its count; each failure is a row
 *  of its own. */
export function downloadChanges(
	items: readonly DownloadOutcome[],
	seen: Set<string>
): SourceChange[] {
	const out: SourceChange[] = [];
	for (const it of items) {
		if ((it.status !== 'done' && it.status !== 'error') || seen.has(it.id)) continue;
		seen.add(it.id);
		if (it.status === 'done') {
			out.push({
				op: 'raise',
				input: {
					key: `download-done:${it.title}`,
					kind: 'download-done',
					count: it.rangeTotal ?? 1,
					data: { title: it.title, destDir: it.destDir }
				}
			});
		} else {
			out.push({
				op: 'raise',
				input: {
					key: `download-failed:${it.startedAt}:${it.id}`,
					kind: 'download-failed',
					data: { title: it.title, episode: it.episode, reason: it.error }
				}
			});
		}
	}
	return out;
}
