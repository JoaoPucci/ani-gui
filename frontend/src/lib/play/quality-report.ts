/**
 * What the engine reports, read as the quality governor needs it: an
 * error as a stall, a level that failed or a level's segments that
 * failed; an init segment as the height a level is encoded at.
 */

import type { ErrorData, ErrorDetails, PlaylistLevelType } from 'hls.js';
import type { QualityGovernor, QualityMove } from './quality-governor';

// hls.js's own names, spelled out so the engine double of a test that
// mocks the module still meets them, and typed against the engine's
// enums so a renamed one is a compile error.
const MAIN: `${PlaylistLevelType.MAIN}` = 'main';
const BUFFER_STALLED: `${ErrorDetails.BUFFER_STALLED_ERROR}` = 'bufferStalledError';
const levelFailures: `${ErrorDetails}`[] = [
	'levelLoadError',
	'levelLoadTimeOut',
	'levelParsingError',
	'levelEmptyError'
];
const fragmentFailures: `${ErrorDetails}`[] = [
	'fragLoadError',
	'fragLoadTimeOut',
	'fragParsingError'
];
/** A level's playlist failing. */
const LEVEL_FAILURES: ReadonlySet<string> = new Set(levelFailures);
/** A level's segments failing. */
const FRAGMENT_FAILURES: ReadonlySet<string> = new Set(fragmentFailures);

export type QualityReport =
	| { kind: 'stall' }
	| { kind: 'level-failed'; level: number }
	| { kind: 'fragments-failed'; level: number };

/** Whether a fragment is of the main rendition — the video the levels
 *  are levels of. */
export function isMain(frag: { type?: unknown } | undefined): boolean {
	return String(frag?.type) === MAIN;
}

/** An engine error as the governor weighs it, or null when it is none
 *  of its business. A stall counts unless the element is seeking; a
 *  failure counts once the engine has given up on it — switched away
 *  from the level, or stopped. */
export function readError(
	data: Pick<ErrorData, 'details' | 'fatal' | 'levelRetry' | 'level' | 'context' | 'frag'>,
	seeking: boolean
): QualityReport | null {
	const details: string = data.details;
	if (details === BUFFER_STALLED && !data.fatal) return seeking ? null : { kind: 'stall' };
	const verdict = data.fatal || data.levelRetry === true;
	if (LEVEL_FAILURES.has(details) && verdict) {
		const level = data.level ?? data.context?.level;
		return typeof level === 'number' ? { kind: 'level-failed', level } : null;
	}
	if (FRAGMENT_FAILURES.has(details) && verdict && isMain(data.frag)) {
		const level = data.frag?.level;
		return typeof level === 'number' ? { kind: 'fragments-failed', level } : null;
	}
	return null;
}

/** The governor's answer to a report. */
export function reactTo(
	governor: QualityGovernor,
	report: QualityReport | null,
	now: number
): QualityMove | null {
	if (!report) return null;
	if (report.kind === 'stall') return governor.stall(now);
	if (report.kind === 'level-failed') return governor.levelFailed(report.level);
	return governor.fragmentsFailed(report.level, now);
}

/** What a parsed init segment carries, as far as it is read: hls.js
 *  types the event's data as empty, and hands over the segment's
 *  fragment and the tracks the demuxer found, with the picture's size
 *  the stream's own parameter sets gave. */
interface InitSegment {
	frag?: { type?: string; level?: number };
	tracks?: Partial<Record<'video' | 'audiovideo', { metadata?: { height?: number } }>>;
}

/** The height a main-rendition init segment says its level is encoded
 *  at, or null when it says none. */
export function readInitSegment(data: unknown): { level: number; height: number } | null {
	const { frag, tracks } = data as InitSegment;
	const level = frag?.level;
	if (!isMain(frag) || level === undefined) return null;
	const height = tracks?.video?.metadata?.height ?? tracks?.audiovideo?.metadata?.height;
	return height ? { level, height } : null;
}

/** The governor's answer to a segment buffered: only the video's
 *  segments say anything about the level. */
export function healthyMove(
	governor: QualityGovernor,
	frag: { type?: unknown },
	now: number,
	bufferAheadS: number
): QualityMove | null {
	return isMain(frag) ? governor.healthy(now, bufferAheadS) : null;
}

/** The page's clock. */
export function pageNow(): number {
	return performance.now();
}

/** The debug log the renderer forwards to the app's log. */
export function consoleQualityLog(message: string): void {
	console.debug(`[play] quality: ${message}`);
}
