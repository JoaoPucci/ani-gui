// The play page's video element, as a scenario reaches it: the page
// makes its own when it mounts and puts it in the player's slot.

/** The mounted play page's video element; throws while there is none. */
export function playerVideo(): HTMLVideoElement {
	const video = document.querySelector<HTMLVideoElement>('.player-video-slot video');
	if (!video) throw new Error('no video in the player slot');
	return video;
}

/** Whether the play page has put its video in the player's slot. */
export function playerVideoInSlot(): boolean {
	return document.querySelector('.player-video-slot video') !== null;
}
