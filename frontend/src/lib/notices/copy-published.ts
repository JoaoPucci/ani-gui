/** The words for a published notice, and the developer's words and
 *  link wherever they appear (./copy): drawn as text, the link shown as
 *  its address and opened by the row's one action. */
import { m } from '$lib/paraglide/messages';
import type { Action, Block, NoticeCopy } from './copy';
import type { Notice } from './model';

const str = (v: unknown): string => (typeof v === 'string' ? v : '');

/** A published notice: the maintainer's words, as text. */
export function published(n: Notice): NoticeCopy {
	const body = str(n.data.body);
	const link = str(n.data.link);
	return {
		title: str(n.data.title),
		meta: m.notices_published_meta(),
		body: [...(body ? [{ label: null, text: body }] : []), ...linkBlock(link)],
		action: linkAction(link)
	};
}

/** A link shown as its address, so the user sees where it goes. */
export function linkBlock(link: string): Block[] {
	return link ? [{ label: m.notices_published_link_label(), text: link }] : [];
}

export function linkAction(link: string): Action | null {
	return link ? { kind: 'open-link', label: m.notices_published_link_action() } : null;
}
