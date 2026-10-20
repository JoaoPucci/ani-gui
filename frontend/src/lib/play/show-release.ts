/**
 * The shows a show page releases when the viewer leaves it.
 *
 * The detail and player pages warm plays for their show into the play
 * cache, and release them when the viewer leaves, so abandoned
 * resolves stop holding the provider's slots. The pages are adapters:
 * they report each show they show, and call `leave` as they are
 * destroyed.
 *
 * The show is taken when the page reports it, not when it leaves. By
 * the time a page's cleanup runs, the route already names where the
 * viewer went — no show for home, another show for that show's page —
 * and releasing that would leave the abandoned show's plays running
 * and cancel the warms of the page the viewer is arriving at.
 *
 * Every show is kept until the page goes. The routes keep their
 * component across a move to another show, and a play started for the
 * first show still answers there (it records that show's watch), so
 * its release waits for the page to be left.
 */

export interface ShowReleases {
	/** The page is showing the show `showId` names now. */
	showing(showId: () => string): void;
	/** The page is going: release every show it showed. */
	leave(): void;
}

export function createShowReleases(release: (id: string) => void): ShowReleases {
	const shown = new Set<string>();
	return {
		showing(showId) {
			const id = showId();
			if (id) shown.add(id);
		},
		leave() {
			for (const id of shown) release(id);
			shown.clear();
		}
	};
}
