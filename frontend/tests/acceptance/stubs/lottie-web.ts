// lottie-web crashes on module init under happy-dom: it grabs a 2D
// canvas context that isn't there and sets fillStyle on null. The
// loading overlay imports it, so any scenario that renders the
// overlay takes the whole run down with an unhandled rejection —
// tests pass, exit code is 1.
//
// The animation is decoration. What scenarios assert is that the
// overlay is mounted, so a no-op player is the whole contract.
export default {
	loadAnimation: () => ({
		destroy: () => {},
		play: () => {},
		stop: () => {},
		setSpeed: () => {},
		addEventListener: () => {},
		removeEventListener: () => {}
	}),
	setQuality: () => {}
};
