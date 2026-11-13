/** One stored row, checked field by field: a stored event, or null. */
import { isKind, lifetimeOf, type Notice, type NoticeData } from './kinds';

export function storedRow(r: unknown): Notice | null {
	if (typeof r !== 'object' || r === null) return null;
	const o = r as Record<string, unknown>;
	const num = (v: unknown) => typeof v === 'number' && Number.isFinite(v);
	if (typeof o.key !== 'string' || !isKind(o.kind) || lifetimeOf(o.kind) !== 'event') return null;
	if (!num(o.firstAt) || !num(o.at) || !num(o.count) || typeof o.read !== 'boolean') return null;
	return {
		key: o.key,
		kind: o.kind,
		firstAt: o.firstAt as number,
		at: o.at as number,
		count: o.count as number,
		read: o.read,
		resolvedAt: null,
		data: storedData(o.data)
	};
}

function storedData(v: unknown): NoticeData {
	if (typeof v !== 'object' || v === null || Array.isArray(v)) return {};
	const out: Record<string, string | number | null> = {};
	for (const [k, x] of Object.entries(v)) {
		if (typeof x === 'string' || x === null || (typeof x === 'number' && Number.isFinite(x))) {
			out[k] = x;
		}
	}
	return out;
}
