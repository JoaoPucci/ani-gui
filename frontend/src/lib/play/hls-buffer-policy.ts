/**
 * Buffer bounds for the player engine.
 *
 * Behind the playhead: hls.js keeps everything already played by
 * default (`backBufferLength: Infinity`), so the page process grows
 * for as long as an episode plays, up to Chromium's own per-source
 * cap. Measured on a 1080p stream, five minutes in, the media pipeline
 * held four and a half minutes of video, about 235 MB, and the page
 * process was about 250 MB above an idle page. The policy
 * keeps 30 s behind the playhead and lets the rest go, the window
 * mature players settle on: Shaka Player's `bufferBehind` and
 * video.js's back buffer both default to 30 s, dash.js keeps 20 s.
 * That covers several arrow-key seeks back (5 s each) without asking
 * the host again; a longer jump back loads its fragments once more, as
 * a seek forward past the buffer always has. hls.js trims on whole
 * target durations and never closer than one target duration, and the
 * media source cuts at a keyframe, so what stays is about 30 s, not
 * exactly that.
 *
 * Ahead of the playhead: left at the engine's defaults. That buffer
 * is the runway the stall handling waits on through a slow host, and
 * the engine already bounds what it fills toward by size
 * (`maxBufferSize`, 60 MB) as well as by time: `maxBufferLength`
 * (30 s), or what 60 MB holds at the level's bitrate when that is
 * longer, never past `maxMaxBufferLength` (600 s). On the 1080p
 * stream measured, that came to about a minute and a half ahead.
 */

import type { HlsConfig } from 'hls.js';

/** Typed against hls.js's own configuration, like the load policy, so
 *  a misspelled key is a compile error rather than a setting the
 *  engine silently ignores. */
export const HLS_BUFFER_POLICY: Pick<HlsConfig, 'backBufferLength'> = {
	backBufferLength: 30
};
