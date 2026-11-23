// lottie-web crashes on module init under happy-dom: it grabs a 2D
// canvas context that isn't there and sets fillStyle on null. The
// loading overlay imports it, so any scenario that renders the
// overlay takes the whole run down with an unhandled rejection —
// tests pass, exit code is 1.
//
// The animation is decoration. What scenarios assert is that the
// overlay is mounted, and, under reduced motion, that the loader never
// played — so the player is a no-op that writes down what it was asked
// to do.

/** What each player was asked to do, across the run, in order:
 *  `load:autoplay` or `load:still` when created, then `play`, `pause`,
 *  `stop` or `hold:<frame>`. */
export const lottieLog: string[] = [];

export default {
	loadAnimation: (options: { autoplay?: boolean }) => {
		lottieLog.push(options.autoplay === false ? 'load:still' : 'load:autoplay');
		return {
			totalFrames: 76,
			destroy: () => {},
			play: () => lottieLog.push('play'),
			pause: () => lottieLog.push('pause'),
			stop: () => lottieLog.push('stop'),
			goToAndStop: (value: number) => lottieLog.push(`hold:${value}`),
			setSpeed: () => {},
			addEventListener: () => {},
			removeEventListener: () => {}
		};
	},
	setQuality: () => {}
};
