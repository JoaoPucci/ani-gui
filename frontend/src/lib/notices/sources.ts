/**
 * What raises a notice: the update check, the download list, and the
 * source's health as the backend reads it from the app's own traffic.
 * Each is turned into keyed changes here; none makes a request.
 */
import type { NoticeData, RaiseInput } from './model';
import type { ProviderHealth } from './health-answer';

export type SourceChange =
	| { op: 'raise'; input: RaiseInput }
	| { op: 'resolve'; key: string; at?: number; data: NoticeData }
	| { op: 'withdraw'; key: string };

export const UPDATE_KEY = 'update';

/** The update row: raised while a newer release exists, withdrawn
 *  once it does not (the update was installed). */
export function updateChange(tag: string | null, current: string): SourceChange {
	if (tag === null) return { op: 'withdraw', key: UPDATE_KEY };
	return { op: 'raise', input: { key: UPDATE_KEY, kind: 'update', data: { tag, current } } };
}

export { parseHealth, type ProviderHealth } from './health-answer';

export const outageKey = (provider: string) => `outage:${provider}`;

/** The outage rows the providers' health asks for: raised while a
 *  provider is not answering, resolved once it answers again or is
 *  plainly answering. */
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
		// Plainly answering: a live row (one the backend has since
		// forgotten, after a restart) is over as of now.
		return [{ op: 'resolve', key, data: {} }];
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
