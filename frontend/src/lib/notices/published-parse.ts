/** The backend's `/api/notices` answer, read strictly again (see
 *  ./published): a notice whose fields are not what they should be is
 *  dropped, and one id is kept once. */
import { optionalText, plainLink, text } from './published-fields';
import type { Priority, Published, PublishedAnswer } from './published';

const NOTHING: PublishedAnswer = { donationAsk: null, items: [] };
const PRIORITIES: readonly string[] = ['info', 'warning', 'critical'];

/** The backend's answer, each notice checked field by field. */
export function parsePublished(raw: unknown): PublishedAnswer {
	if (typeof raw !== 'object' || raw === null || Array.isArray(raw)) return NOTHING;
	const o = raw as Record<string, unknown>;
	const n = o.donation_ask;
	const donationAsk = typeof n === 'number' && Number.isInteger(n) && n >= 0 ? n : null;
	const items: Published[] = [];
	for (const r of Array.isArray(o.items) ? o.items : []) {
		const p = notice(r);
		if (p && !items.some((i) => i.id === p.id)) items.push(p);
	}
	return { donationAsk, items };
}

function notice(r: unknown): Published | null {
	if (typeof r !== 'object' || r === null) return null;
	const o = r as Record<string, unknown>;
	const priority = o.priority ?? 'info';
	if (!text(o.id) || !text(o.title) || !optionalText(o.body)) return null;
	if (typeof priority !== 'string' || !PRIORITIES.includes(priority)) return null;
	const link = o.link == null ? null : plainLink(o.link);
	if (o.link != null && link === null) return null;
	return {
		id: o.id as string,
		title: o.title as string,
		body: (o.body as string | null | undefined) ?? null,
		link,
		priority: priority as Priority,
		aboutSource: o.about_source === true
	};
}
