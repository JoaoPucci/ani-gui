import { describe, expect, it } from 'vitest';
import { rewatchNoteShown, statusOptionsForTrackers } from './rewatch-trackers';
import { STATUS_OPTIONS } from './list-entry-edit';

// MyAnimeList has no rewatch status: a row it reads as rewatching is listed
// as Watching, and "you finished this show" lives only in Completed.
describe('statusOptionsForTrackers', () => {
	it('offers no Rewatching when no connected tracker can hold one', () => {
		expect(statusOptionsForTrackers(STATUS_OPTIONS, ['mal'], 'completed')).not.toContain(
			'rewatching'
		);
	});

	it('still shows Rewatching where the entry already is one', () => {
		expect(statusOptionsForTrackers(STATUS_OPTIONS, ['mal'], 'rewatching')).toContain('rewatching');
	});

	it('offers Rewatching while a connected tracker can hold it', () => {
		expect(statusOptionsForTrackers(STATUS_OPTIONS, ['anilist', 'mal'], 'completed')).toEqual(
			STATUS_OPTIONS
		);
		expect(statusOptionsForTrackers(STATUS_OPTIONS, ['anilist'], null)).toEqual(STATUS_OPTIONS);
	});
});

describe('rewatchNoteShown', () => {
	it('says MyAnimeList keeps its entry when Rewatching is picked with it connected', () => {
		expect(rewatchNoteShown(['anilist', 'mal'], 'rewatching')).toBe(true);
	});

	it('says nothing for any other status, or without MyAnimeList', () => {
		expect(rewatchNoteShown(['anilist', 'mal'], 'completed')).toBe(false);
		expect(rewatchNoteShown(['anilist'], 'rewatching')).toBe(false);
	});
});
