/**
 * Buffer bounds for the player engine.
 */

import type { HlsConfig } from 'hls.js';

/** Typed against hls.js's own configuration, like the load policy, so
 *  a misspelled key is a compile error rather than a setting the
 *  engine silently ignores. */
export const HLS_BUFFER_POLICY: Partial<Pick<HlsConfig, 'backBufferLength'>> = {};
