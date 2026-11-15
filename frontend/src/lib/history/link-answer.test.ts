// What a link session does with a link's answer.

import { describe, expect, it } from 'vitest';
import type { KitsuAnimeRef } from '$lib/api';
import type { LinkNotice, LinkSessionDeps } from './link-session.svelte';
import { linkAnswered, linkFailed, type LinkAsked } from './link-answer';

const ref = (id: string) => ({ id, canonical_title: `Show ${id}` }) as KitsuAnimeRef;
const OFFER = { entry: { id: 'hianime:x-1', title: 'X', ep_no: '1' }, current: ref('1') };

function asked(stayed: boolean, onSurface = stayed, from = '1') {
	const done: string[] = [];
	const notices: LinkNotice[] = [];
	const deps = {
		notify: (n: LinkNotice) => notices.push(n),
		refresh: () => done.push('refresh'),
		showPage: (id: string) => done.push(`page:${id}`)
	} as unknown as LinkSessionDeps;
	const a: LinkAsked = {
		session: { open: true, offer: OFFER, error: null },
		deps,
		offer: OFFER,
		chosen: ref('2'),
		from,
		stayed: () => stayed,
		onSurface: () => onSurface
	};
	return { a, done, notices };
}

const saved = { kitsu_id: '2', linked: [] };

describe('linkAnswered', () => {
	it('says the show was saved and opens its page, for a viewer who stayed', () => {
		const { a, done, notices } = asked(true);
		expect(linkAnswered(saved, a)).toBe(true);
		expect(done).toEqual(['page:2']);
		expect(a.session).toMatchObject({ open: false, offer: null });
		expect(notices).toHaveLength(1);
	});

	it('reads the page again first when the link is to the show already open', () => {
		const { a, done } = asked(true, true, '2');
		linkAnswered(saved, a);
		expect(done).toEqual(['refresh', 'page:2']);
	});

	it('reads the history again on a surface moved on to another card', () => {
		const { a, done } = asked(false, true);
		expect(linkAnswered(saved, a)).toBe(true);
		expect(done).toEqual(['refresh']);
		expect(a.session.open).toBe(true);
	});

	it('changes nothing on screen for a viewer who left, but still says it', () => {
		const { a, done, notices } = asked(false, false);
		expect(linkAnswered(saved, a)).toBe(true);
		expect(done).toEqual([]);
		expect(notices).toHaveLength(1);
	});

	it('says nothing for a link a later one overtook, and reads the history again', () => {
		const overtaken = { ...saved, overtaken: true };
		const stayed = asked(true);
		expect(linkAnswered(overtaken, stayed.a)).toBe(false);
		expect(stayed.done).toEqual(['refresh']);
		expect(stayed.a.session).toMatchObject({ open: false, offer: OFFER });
		expect(stayed.notices).toEqual([]);
		const left = asked(false, false);
		expect(linkAnswered(overtaken, left.a)).toBe(false);
		expect(left.done).toEqual([]);
		expect(left.a.session.open).toBe(true);
	});
});

describe('an overtaken link on a surface that reads its line again its own way', () => {
	it('re-reads the line rather than the page', () => {
		const { a, done } = asked(true);
		(a.deps as unknown as { reread: () => void }).reread = () => done.push('reread');
		linkAnswered({ ...saved, overtaken: true }, a);
		expect(done).toEqual(['reread']);
	});
});

describe('linkFailed', () => {
	it('says why in the picker for a viewer who stayed, else in a notice', () => {
		const stayed = asked(true);
		expect(linkFailed(new Error('down'), stayed.a)).toBe(false);
		expect(stayed.a.session.error).toEqual(expect.any(String));
		expect(stayed.notices).toEqual([]);
		const left = asked(false);
		linkFailed(new Error('down'), left.a);
		expect(left.a.session.error).toBeNull();
		expect(left.notices).toEqual([{ kind: 'error', message: expect.any(String) }]);
	});
});
