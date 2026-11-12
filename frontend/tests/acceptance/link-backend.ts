// A fake of the backend's history link routes for the acceptance
// tests of the change-show flows: the history, the link and the group a link from a card
// would write.
//
// The group's members are the rows the backend finds the card stands
// for from each row's match (link_verdict.rs `group_members`, faked in
// frontend/src/test/link-rules-fake.ts). Of those, it answers as the backend does
// (history_link_plan.rs,
// `card_listing`): a row shares the card's listing when it is the
// card's own row, or when one of the two is a bare row of a retired
// provider whose slug carries the same words as the other's. Two rows
// of a walked provider under different ids are two listings — JoJo's
// 2000 OVA and its 2012 series — and so are two bare rows.

import { http, HttpResponse, type HttpHandler } from 'msw';
import { API_BASE } from './setup';
import { groupMembers } from '../../src/test/link-rules-fake';
import type { LinkGroupRequest } from '../../src/lib/api';

export interface Row {
	ep_no: string;
	id: string;
	title: string;
	kitsu_id?: string;
	/** The card whose guessed play wrote this row on another listing. */
	guessed_from?: string;
	/** The row records a show the user named for it. */
	named?: boolean;
}

export interface LinkBackend {
	rows: Row[];
	linkCalls: { card: string; rows: string[]; kitsu_id: string }[];
}

export function linkBackend(rows: Row[]): LinkBackend {
	return { rows, linkCalls: [] };
}

const walked = (id: string) => id.startsWith('hianime:');
const words = (id: string) =>
	id
		.replace(/^hianime:/, '')
		.match(/^(.+)-\d+$/)?.[1]
		?.toLowerCase() ?? null;

/** Whether `row` shares the listing of the card whose row is `card`. */
export function cardListing(card: string, row: string): boolean {
	if (card === row) return true;
	if (walked(card) === walked(row)) return false;
	const a = words(card);
	return a !== null && a === words(row);
}

function held(b: LinkBackend, card: string, rows: string[]): string[] {
	return rows.filter(
		(r, i) => rows.indexOf(r) === i && cardListing(card, r) && b.rows.some((h) => h.id === r)
	);
}

export function linkBackendHandlers(b: LinkBackend): HttpHandler[] {
	return [
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json(b.rows)),
		http.get(`${API_BASE}/api/history/:id/number`, ({ params }) => {
			const row = b.rows.find((r) => r.id === params.id);
			return HttpResponse.json(row ? { ep_no: row.ep_no } : null);
		}),
		http.post(`${API_BASE}/api/history/link-group`, async ({ request }) => {
			const body = (await request.json()) as LinkGroupRequest;
			const members = groupMembers(body.card, body.show, body.saved, body.rows);
			return HttpResponse.json(held(b, body.card, members));
		}),
		http.post(`${API_BASE}/api/history/link`, async ({ request }) => {
			const body = (await request.json()) as { card: string; rows: string[]; kitsu_id: string };
			b.linkCalls.push(body);
			const ids = held(b, body.card, body.rows);
			const linked = b.rows
				.filter((r) => ids.includes(r.id))
				.map((r) => ({
					row: r.id,
					previous: r.kitsu_id ?? null,
					ep_no: r.ep_no,
					watched_at: null
				}));
			b.rows = b.rows.map((r) =>
				ids.includes(r.id) ? { ...r, kitsu_id: body.kitsu_id, named: true } : r
			);
			return HttpResponse.json({ kitsu_id: body.kitsu_id, linked });
		})
	];
}
