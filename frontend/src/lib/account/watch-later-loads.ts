// Which of Home's Watch Later loads is the latest, and which owes the
// row's one-shot freshness check once it lands. Kept out of
// `routes/+page.svelte` so the page stays a thin adapter (AGENTS.md §2).

/** A load's place in line, from {@link RailLoads.start}. */
export type LoadTicket = { readonly seq: number };

export interface RailLoads {
	/** A load starts. `checksFreshness` when it should run the freshness
	 *  check once it lands — the load the row opens with. */
	start(checksFreshness: boolean): LoadTicket;
	/** Whether no newer load has started since `ticket`'s. */
	current(ticket: LoadTicket): boolean;
	/** `ticket`'s load landed: whether it is to run the freshness check
	 *  now. Never for a load a newer one superseded; for the latest, when
	 *  any load since the last check ran asked for one — a reread that
	 *  supersedes the opening load, or follows one that failed, runs the
	 *  check that load owed. A check whose loads all fail stays owed
	 *  until the next load lands. Once per check asked for. */
	landed(ticket: LoadTicket): boolean;
}

export function createRailLoads(): RailLoads {
	let seq = 0;
	let checkOwed = false;
	return {
		start(checksFreshness) {
			seq += 1;
			if (checksFreshness) checkOwed = true;
			return { seq };
		},
		current: (ticket) => ticket.seq === seq,
		landed(ticket) {
			if (ticket.seq !== seq || !checkOwed) return false;
			checkOwed = false;
			return true;
		}
	};
}
