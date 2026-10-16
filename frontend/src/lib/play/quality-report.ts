/**
 * What the engine reports, read as the quality governor needs it: an
 * error as a stall, a level that failed or a level's segments that
 * failed; an init segment as the height a level is encoded at.
 */

import type { ErrorData, ErrorDetails, NetworkErrorAction } from 'hls.js';
import type { QualityGovernor, QualityMove } from './quality-governor';
import { isMain, isSideRendition } from './quality-rendition';

// hls.js's own names, spelled out so the engine double of a test that
// mocks the module still meets them, and typed against the engine's
// enums so a renamed one is a compile error.
const BUFFER_STALLED: `${ErrorDetails.BUFFER_STALLED_ERROR}` = 'bufferStalledError';
const levelFailures: `${ErrorDetails}`[] = [
	'levelLoadError',
	'levelLoadTimeOut',
	'levelParsingError',
	'levelEmptyError'
];
/** A level's playlist failing. */
const LEVEL_FAILURES: ReadonlySet<string> = new Set(levelFailures);
/** The error controller's verdict that it gave up on a level and sent
 *  the engine to another; a retry still pending is `RetryRequest`. */
const SWITCHED_AWAY: NetworkErrorAction.SendAlternateToPenaltyBox = 2;

export type QualityReport =
	| { kind: 'stall' }
	| { kind: 'level-failed'; level: number }
	/** `level` null: the error named none, so the level held failed. */
	| { kind: 'fragments-failed'; level: number | null }
	/** hls.js switched to `level` over a side rendition's failure. */
	| { kind: 'followed'; level: number };

export { isMain } from './quality-rendition';

/** An engine error as the governor weighs it, or null when it is none
 *  of its business. A stall counts unless the element is seeking. A
 *  failure counts once the engine has given up on it: switched to
 *  another level, or — for a playlist or a video segment — stopped.
 *  A retry still pending is no verdict, though the level controller
 *  flags a playlist's as `levelRetry`. A switch over a side rendition
 *  — audio or subtitles — is followed; any other switch away is a
 *  failure of the level it names, or of the level held when it names
 *  none, so the governor steps down from it rather than putting the
 *  engine back. */
export function readError(
	data: Pick<
		ErrorData,
		'details' | 'fatal' | 'errorAction' | 'level' | 'context' | 'frag' | 'sourceBufferName'
	>,
	seeking: boolean
): QualityReport | null {
	const details: string = data.details;
	if (details === BUFFER_STALLED && !data.fatal) return seeking ? null : { kind: 'stall' };
	const action = data.errorAction;
	const next = action?.action === SWITCHED_AWAY ? action.nextAutoLevel : undefined;
	if (next === undefined && !data.fatal) return null;
	if (LEVEL_FAILURES.has(details)) {
		const level = data.level ?? data.context?.level;
		return typeof level === 'number' ? { kind: 'level-failed', level } : null;
	}
	// An audio or subtitle segment, playlist or buffer: the engine's
	// switch is over a side rendition, and the video level follows it
	// rather than failing.
	if (isSideRendition(data)) {
		return next === undefined ? null : { kind: 'followed', level: next };
	}
	// The level the failure names: the video segment's, or `data.level`,
	// which hls.js's own switch reads before falling back to the level
	// it was loading (`data.level ?? hls.loadLevel`). In hls.js 1.6.19
	// the segment-less errors that reach here — a video buffer refusing
	// its codec — set neither, so the failure is of the level loading,
	// which is the level the governor holds.
	const level = data.frag?.level ?? data.level;
	if (typeof level === 'number') return { kind: 'fragments-failed', level };
	return next === undefined ? null : { kind: 'fragments-failed', level: null };
}

/** The governor's answer to a report. */
export function reactTo(
	governor: QualityGovernor,
	report: QualityReport | null,
	now: number
): QualityMove | null {
	if (!report) return null;
	if (report.kind === 'stall') return governor.stall(now);
	if (report.kind === 'level-failed') return governor.levelFailed(report.level, now);
	if (report.kind === 'followed') return governor.follow(report.level, now);
	return governor.fragmentsFailed(report.level ?? governor.level, now);
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
