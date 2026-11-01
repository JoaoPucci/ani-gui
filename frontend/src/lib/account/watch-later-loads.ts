// Which of Home's Watch Later loads is the latest, and which owes the
// row's one-shot freshness check once it lands. Kept out of
// `routes/+page.svelte` so the page stays a thin adapter (AGENTS.md §2).

/** A load's place in line, from {@link RailLoads.start}. */
export type LoadTicket = { readonly seq: number; readonly checksFreshness: boolean };

export interface RailLoads {
	/** A load starts. `checksFreshness` when it should run the freshness
	 *  check once it lands — the load the row opens with. */
	start(checksFreshness: boolean): LoadTicket;
	/** Whether no newer load has started since `ticket`'s. */
	current(ticket: LoadTicket): boolean;
	/** `ticket`'s load landed: whether it is to run the freshness check
	 *  now. Never for a load a newer one superseded. */
	landed(ticket: LoadTicket): boolean;
}

export function createRailLoads(): RailLoads {
	let seq = 0;
	return {
		start(checksFreshness) {
			seq += 1;
			return { seq, checksFreshness };
		},
		current: (ticket) => ticket.seq === seq,
		landed: (ticket) => ticket.seq === seq && ticket.checksFreshness
	};
}
