/**
 * The shows a show page releases when the viewer leaves it.
 *
 * The detail and player pages warm plays for their show into the play
 * cache, and release them when the viewer leaves, so abandoned
 * resolves stop holding the provider's slots. The pages are adapters:
 * they report the show they are showing, and call `leave` as they are
 * destroyed.
 */

export interface ShowReleases {
	/** The page is showing the show `showId` names. */
	showing(showId: () => string): void;
	/** The page is going: release its show. */
	leave(): void;
}

export function createShowReleases(release: (id: string) => void): ShowReleases {
	let current: () => string = () => '';
	return {
		showing(showId) {
			current = showId;
		},
		leave() {
			const id = current();
			if (id) release(id);
		}
	};
}
