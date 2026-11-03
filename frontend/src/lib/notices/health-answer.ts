/** The backend's `/api/source-health` answer, read defensively: one
 *  entry per provider, anything malformed left out. */

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
