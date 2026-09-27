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
 * — a hundred requests a second until the host refuses the address.
 *
 * The fixture is a two-second encode with that shape: video from
 * 0.184 s, audio from 1.163 s, one program, H.264 and AAC.
 */

const FIXTURE = new URL('./fixtures/video-before-audio.mpegts', import.meta.url);
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

/** Drives the worker bundle in-process, the way a Web Worker would. */
async function transmuxFirstFragment(bytes: Uint8Array): Promise<RemuxResult[]> {
	const posted: Posted[] = [];
	let onMessage: ((e: { data: unknown }) => void) | null = null;
	const g = globalThis as unknown as Record<string, unknown>;
	g.self = globalThis;
	g.postMessage = (msg: Posted) => {
		posted.push(msg);
	};
	g.addEventListener = (type: string, fn: (e: { data: unknown }) => void) => {
		if (type === 'message') onMessage = fn;
	};
	// @ts-expect-error the worker bundle ships without a declaration file
	await import('hls.js/dist/hls.worker.js');
	if (!onMessage) throw new Error('the worker did not register a message handler');
	const send = (data: Record<string, unknown>) => onMessage!({ data: { instanceNo: 1, ...data } });
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
	it('keeps its video: the stream begins at the video, not at the audio', async () => {
		const bytes = new Uint8Array(readFileSync(FIXTURE));
		const results = await transmuxFirstFragment(bytes);
		const init = results.map((r) => r.initSegment).find((s) => s?.initPTS !== undefined);
		expect(init, 'an init segment carrying the stream origin').toBeDefined();
		// The origin is the earliest timestamp of the fragment — the
		// video's — so the video lands from zero. Read against the
		// audio's instead, it lands a second before zero and is lost.
		expect(init!.initPTS! / init!.timescale!).toBeCloseTo(VIDEO_START_S, 2);
		const video = results.map((r) => r.video).find(Boolean);
		expect(video, 'remuxed video').toBeDefined();
		expect(video!.startPTS).toBeGreaterThanOrEqual(0);
		expect(video!.startPTS).toBeLessThan(0.1);
		expect(video!.dropped ?? 0).toBe(0);
		expect(video!.nb).toBeGreaterThan(0);
	});
});
