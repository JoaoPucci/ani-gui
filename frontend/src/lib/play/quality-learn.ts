/**
 * What the player learns of a level's height after its playlist: the
 * height a parsed segment turns out to be encoded at, and, for a
 * stream of one level, the picture's height as the video element
 * reports it.
 */

import { readInitSegment } from './quality-report';
import type { QualityTrack } from './quality-track';

/** A parsed init segment named its level's encoded height: true when
 *  that is news. The log notes a height its playlist listed otherwise. */
export function learnEncoded(
	track: QualityTrack,
	data: unknown,
	log: (message: string) => void
): boolean {
	const news = track.noteEncoded(readInitSegment(data));
	if (!news) return false;
	if (news.listed > 0 && news.listed !== news.height) {
		log(`level ${news.level} encodes ${news.height}p; its playlist says ${news.listed}p`);
	}
	return true;
}

/** The picture's height as the video element reports it, read for a
 *  stream of one level: true when it names that level anew. hls.js
 *  reports no picture size for an fMP4 init segment, so a lone level
 *  whose playlist names no height would otherwise stay unnamed. A
 *  stream of several levels is named by its segments alone: the
 *  element shows one level at a time, and not always the one the
 *  engine last switched to. */
export function learnPicture(track: QualityTrack, height: number): boolean {
	return (
		track.declared().length === 1 && height > 0 && track.noteEncoded({ level: 0, height }) !== null
	);
}
