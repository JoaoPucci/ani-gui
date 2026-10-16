/**
 * Stub: the signatures the tests are written against, with no
 * behaviour behind them yet.
 */

import { availabilityBatch } from '$lib/api';
import { m } from '$lib/paraglide/messages';
import type { OtherAudio } from '$lib/play/error-copy';

type Batch = typeof availabilityBatch;

export function unavailableMessage(mode: string, otherAudio: OtherAudio): string {
	void mode;
	void otherAudio;
	return m.detail_unavailable_message();
}

export function watchOtherAudio(
	q: { kitsuId: string | null | undefined; mode: string },
	onAnswer: (answer: OtherAudio) => void,
	batch: Batch = availabilityBatch
): () => void {
	void q;
	void onAnswer;
	void batch;
	return () => {};
}
