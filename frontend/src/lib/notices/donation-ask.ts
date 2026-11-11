/**
 * The donation ask, wired: the state machine (./donation) fed with the
 * published number and what the window is doing, its state kept in
 * storage. It asks only once it knows both the number and that nothing
 * plays, and it asks at most once a launch. Showing the ask is the
 * caller's (a toast with Donate and Not now); the number is acted on
 * when the ask is shown, so neither choice brings it back.
 */
import { DONATION_STORAGE_KEY, decide, parseState, serializeState } from './donation';

export interface DonationDeps {
	storage: Pick<Storage, 'getItem' | 'setItem'> | null;
	now: () => number;
	show: () => void;
}

export interface DonationAsk {
	/** The published number this launch, or null for none. */
	setNumber(n: number | null): void;
	/** Whether a video plays, or anything else makes this no moment to
	 *  ask (the player is open). */
	setBusy(busy: boolean): void;
}

export function createDonationAsk(deps: DonationDeps): DonationAsk {
	let n: number | null | undefined;
	let busy: boolean | undefined;
	let asked = false;

	function step() {
		if (n === undefined || busy === undefined || asked) return;
		const r = decide(read(), { n, now: deps.now(), playing: busy });
		write(serializeState(r.state));
		if (r.ask) {
			asked = true;
			deps.show();
		}
	}

	function read() {
		try {
			return parseState(deps.storage?.getItem(DONATION_STORAGE_KEY) ?? null);
		} catch {
			return null;
		}
	}

	function write(raw: string) {
		try {
			deps.storage?.setItem(DONATION_STORAGE_KEY, raw);
		} catch {
			// Refused: the ask may come again next launch, never twice in this one.
		}
	}

	return {
		setNumber(value) {
			n = value;
			step();
		},
		setBusy(value) {
			busy = value;
			step();
		}
	};
}
