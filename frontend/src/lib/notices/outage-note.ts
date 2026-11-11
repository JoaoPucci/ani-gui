/**
 * The published note about the source (./published `sourceNote`)
 * joined to the outage rows the source's health raises, whichever of
 * the two answers arrives first. ./NoticeHost hands each answer in and
 * passes the changes on to the center.
 */
import { sourceNote, withSourceNote, type Published, type SourceNote } from './published';
import { outageChanges, type ProviderHealth, type SourceChange } from './sources';

export interface OutageNote {
	/** The published answer arrived: the last health answer's changes
	 *  with the note joined, or nothing while no health is known. */
	published(items: readonly Published[]): SourceChange[];
	/** A health answer arrived: its outage changes, the note joined. */
	health(h: ProviderHealth[]): SourceChange[];
}

export function createOutageNote(): OutageNote {
	let note: SourceNote | null = null;
	let known: ProviderHealth[] | null = null;
	return {
		published(items) {
			note = sourceNote(items);
			return known ? withSourceNote(outageChanges(known), note) : [];
		},
		health(h) {
			known = h;
			return withSourceNote(outageChanges(h), note);
		}
	};
}
