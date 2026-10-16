/**
 * Stub: the signatures the tests are written against, with no
 * behaviour behind them yet.
 */

import { availabilityBatch } from '$lib/api';
import { describePlayFailure, type OtherAudio, type PlayFailureContext } from './error-copy';

export interface OtherAudioQuery {
	kitsuId: string | null | undefined;
	mode: string;
	episode: number | null;
}

type Batch = typeof availabilityBatch;

export function otherAudioMode(mode: string): 'sub' | 'dub' {
	void mode;
	return 'sub';
}

export async function lookupOtherAudio(
	q: OtherAudioQuery,
	batch: Batch = availabilityBatch
): Promise<OtherAudio> {
	void q;
	void batch;
	return null;
}

export async function otherAudioForFailure(
	e: unknown,
	q: OtherAudioQuery,
	batch: Batch = availabilityBatch
): Promise<OtherAudio> {
	void e;
	void q;
	void batch;
	return null;
}

export async function describePlayFailureFromCache(
	e: unknown,
	q: OtherAudioQuery,
	opts: Omit<PlayFailureContext, 'otherAudio'> = {},
	batch: Batch = availabilityBatch
): Promise<string> {
	void q;
	void batch;
	return describePlayFailure(e, { otherAudio: null, ...opts });
}
