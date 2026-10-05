/**
 * The play page's video element. The page makes one when it mounts
 * and releases it when it leaves: the stream stops and unloads, and
 * picture-in-picture, which shows this element, ends with it.
 *
 * What carries from one page to the next is the listener's volume and
 * mute, kept for the app's lifetime and applied to each new element.
 */

let volume = 1;
let muted = false;

export function createPlayerVideo(): HTMLVideoElement {
	const video = document.createElement('video');
	video.autoplay = true;
	video.preload = 'auto';
	// Every source and sidecar track this element loads comes from
	// the proxy, which is another origin from the page on every build
	// (a loopback port against the app's own origin). The browser
	// refuses a <track> from another origin outright unless the media
	// element asks in CORS mode; the proxy answers every route with a
	// permissive allow-origin, so anonymous mode is what makes the
	// sidecar tracks loadable. Media sources are unaffected: hls.js
	// feeds MSE, and the mp4 route carries the same header.
	video.setAttribute('crossorigin', 'anonymous');
	video.style.inlineSize = '100%';
	video.style.blockSize = '100%';
	video.style.display = 'block';
	video.style.background = '#000';
	video.volume = volume;
	video.muted = muted;
	video.addEventListener('volumechange', () => {
		volume = video.volume;
		muted = video.muted;
	});
	return video;
}

/** Puts `video` in `slot`, where it stays until it is released;
 *  a slot that already holds it is left as it is. */
export function placePlayerVideo(video: HTMLVideoElement, slot: HTMLElement): HTMLVideoElement {
	if (video.parentElement !== slot) slot.appendChild(video);
	return video;
}

/** Stops and unloads `video` and takes it out of the page, closing
 *  picture-in-picture first when it is the element shown there. */
export function releasePlayerVideo(video: HTMLVideoElement): void {
	if (document.pictureInPictureElement === video) {
		void document.exitPictureInPicture().catch(() => {});
	}
	video.removeAttribute('src');
	video.load();
	video.remove();
}

/** Test-only: forget the carried volume and mute. */
export function __resetPlayerAudioForTests(): void {
	volume = 1;
	muted = false;
}
