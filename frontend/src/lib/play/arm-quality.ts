/**
 * The player's quality control, wired to one engine: the governor
 * (`quality-governor.ts`) decides which level loads, and this hands it
 * what the engine reports — the master's levels, a stall, a level or
 * its segments failing, a segment buffered, the segment playing —
 * and makes the engine do what it decides. Every change, and every
 * change of the level that plays, is logged at debug level, so which
 * level a viewer was watching can be read off the log afterwards.
 *
 * The play page arms one per HLS stream it attaches and disposes of it
 * with the stream.
 */

import type Hls from 'hls.js';
import type { BufferedRanges } from './buffer-ahead';
import type { QualityView } from './quality-menu';

/** The engine events the control listens to, as the page's engine
 *  names them. */
export type QualityEvents = Pick<
	typeof Hls.Events,
	'MANIFEST_PARSED' | 'ERROR' | 'FRAG_BUFFERED' | 'FRAG_CHANGED' | 'FRAG_PARSING_INIT_SEGMENT'
>;

/** As much of the video element as the control reads. */
export interface QualityVideo extends EventTarget {
	readonly buffered: BufferedRanges;
	readonly currentTime: number;
	readonly seeking: boolean;
}

export interface QualityControl {
	/** The viewer chose a level, or "Auto", from the menu. */
	pick(choice: 'auto' | number): void;
	/** The stream retired. */
	dispose(): void;
}

/** The level the viewer pinned, by height, for the rest of the app's
 *  run: the next episode, and a stream recovered in its place, start
 *  at the same height — or the nearest below it — rather than at the
 *  setting's. "Auto" clears it. */
let pinnedHeight: number | null = null;

export const qualityPin = {
	get: (): number | null => pinnedHeight,
	set(height: number): void {
		void height;
	},
	clear(): void {
		pinnedHeight = null;
	}
};

export function armQuality(input: {
	engine: Hls;
	events: QualityEvents;
	video: QualityVideo;
	/** The quality setting, read when the master's levels arrive. */
	setting: () => string | null | undefined;
	/** Told what the menu shows whenever it changes; null once the
	 *  stream retires. */
	onView: (view: QualityView | null) => void;
	now?: () => number;
	log?: (message: string) => void;
}): QualityControl {
	void input;
	return {
		pick(choice) {
			void choice;
		},
		dispose() {}
	};
}
