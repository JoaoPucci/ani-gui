import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import Hls, { ChunkMetadata } from 'hls.js';

/**
 * The player's demuxer against a first fragment whose video starts
 * before its audio.
 *
 * The provider's encodes open with video at a fraction of a second
 * and audio around a second later. hls.js takes the earliest
 * timestamp of the first fragment as the stream's origin; a version
 * that misreads a video timestamp under one second as a 33-bit
 * rollover picks the audio's instead, every video frame of that
 * fragment lands before zero and is dropped by the media source, and
 * the engine asks for the fragment again as fast as the host answers
 * — well over a hundred requests a second until the host refuses the
 * address.
 *
 * The fixture is a 2.4-second synthesized encode with that shape —
 * video from 0.184 s, audio from 1.163 s, one program, H.264 and AAC —
 * kept as base64 text in the manifest-backed fixture store, whose
 * manifest carries the command that made it and its checksum.
 */

const FIXTURE = new URL(
	'../../../../tests/fixtures/hls/video-before-audio.mpegts.b64',
	import.meta.url
);
const VIDEO_START_S = 0.184;

interface Posted {
	event: string;
	data: unknown;
	instanceNo: number;
}

interface RemuxResult {
	initSegment?: { initPTS?: number; timescale?: number };
	video?: { startPTS: number; endPTS: number; nb: number; dropped?: number };
	audio?: { startPTS: number; endPTS: number };
}

/** The worker bundle, loaded once for the file the way a Web Worker
 *  would load it: it registers one message listener on `self` at
 *  module top level, so the shims that catch it are installed before
 *  the import and the handler is kept for every call after. The
 *  globals stay shimmed for the rest of the file; vitest runs each
 *  test file in its own module graph, so nothing else sees them. */
const posted: Posted[] = [];
const workerHandler: ((e: { data: unknown }) => void) | null = await (async () => {
	let handler: ((e: { data: unknown }) => void) | null = null;
	const g = globalThis as unknown as Record<string, unknown>;
	g.self = globalThis;
	g.postMessage = (msg: Posted) => {
		posted.push(msg);
	};
	g.addEventListener = (type: string, fn: (e: { data: unknown }) => void) => {
		if (type === 'message') handler = fn;
	};
	// @ts-expect-error the worker bundle ships without a declaration file
	await import('hls.js/dist/hls.worker.js');
	return handler;
})();

/** Drives the loaded worker bundle through one fragment: init,
 *  configure, demux, flush — the messages hls.js itself sends. */
function transmuxFirstFragment(bytes: Uint8Array): RemuxResult[] {
	if (!workerHandler) throw new Error('the worker did not register a message handler');
	const onMessage = workerHandler;
	posted.length = 0;
	const send = (data: Record<string, unknown>) => onMessage({ data: { instanceNo: 1, ...data } });
	send({
		cmd: 'init',
		id: 'main',
		typeSupported: { mpeg: false, mp3: false, ac3: false },
		config: JSON.stringify({ ...Hls.DefaultConfig, debug: false })
	});
	send({
		cmd: 'configure',
		config: {
			audioCodec: 'mp4a.40.2',
			videoCodec: 'avc1.42e01e',
			initSegmentData: new Uint8Array(0),
			duration: 2.4,
			defaultInitPts: null
		}
	});
	const chunkMeta = new ChunkMetadata(0, 1, 1, bytes.byteLength, -1, false);
	send({
		cmd: 'demux',
		data: bytes.buffer,
		decryptdata: null,
		chunkMeta,
		state: {
			discontinuity: true,
			contiguous: false,
			accurateTimeOffset: true,
			trackSwitch: false,
			timeOffset: 0,
			initSegmentChange: false
		}
	});
	send({ cmd: 'flush', chunkMeta });
	const errors = posted.filter((p) => p.event === 'hlsError');
	if (errors.length) throw new Error(`the transmuxer reported: ${JSON.stringify(errors[0].data)}`);
	return posted
		.filter((p) => p.event === 'transmuxComplete')
		.map((p) => (p.data as { remuxResult: RemuxResult }).remuxResult);
}

describe('the first fragment of a stream whose video starts before its audio', () => {
	it('keeps its video: the stream begins at the video, not at the audio', () => {
		const bytes = new Uint8Array(Buffer.from(readFileSync(FIXTURE, 'utf8'), 'base64'));
		const results = transmuxFirstFragment(bytes);
		const init = results.map((r) => r.initSegment).find((s) => s?.initPTS !== undefined);
		expect(init, 'an init segment carrying the stream origin').toBeDefined();
		// The origin is the earliest timestamp of the fragment — the
		// video's — so the video lands from zero. Read against the
		// audio's instead, it lands a second before zero and is lost.
		expect(init!.initPTS! / init!.timescale!).toBeCloseTo(VIDEO_START_S, 2);
		const video = results.map((r) => r.video).find(Boolean);
		expect(video, 'remuxed video').toBeDefined();
		// The origin and a non-negative start are what tell the two
		// versions apart: the misread put the video's start a second
		// before zero. The remuxer's own dropped-frame count is zero on
		// both, since the drop it caused happened in the media source.
		expect(video!.startPTS).toBeGreaterThanOrEqual(0);
		expect(video!.startPTS).toBeLessThan(0.1);
		expect(video!.nb).toBeGreaterThan(0);
	});
});
