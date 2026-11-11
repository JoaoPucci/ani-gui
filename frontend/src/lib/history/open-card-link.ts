/**
 * Opening Home's picker for a guessed Continue card — from its pill, or
 * its failed resume's "Pick the right show".
 *
 * A remove or Clear history holds its dialog while it is out, with
 * nothing over it ($lib/history/confirm-run.ts), so nothing opens then.
 * Otherwise the failure overlay, if any, goes, and the card is checked
 * again ($lib/history/card-relink.ts): the picker opens only on a card
 * still guessed, and only if no remove or clear began while the check
 * was out. A check that could not be read says so, and leaves the pill
 * to try again; a card settled since the cards were read opens nothing.
 *
 * Home is an adapter: it hands in the busy flags, the overlay, the
 * check, the notice and the picker.
 */

/** What the check answered: still guessed, no longer, or unknown. */
export type CardCheck = boolean | 'unread';

export interface OpenCardLinkDeps {
	/** Whether a remove or Clear history is out. */
	busy: () => boolean;
	/** Take down the failure overlay. */
	dismissFailure: () => void;
	stillGuessed: (row: string, matchId: string) => Promise<CardCheck>;
	/** Say the card could not be checked. */
	checkFailed: () => void;
	/** Open the picker, naming `episode`. */
	start: (episode: number) => void;
}

/** What came of the opening. */
export type CardLinkOpening = 'held' | 'unread' | 'settled' | 'overtaken' | 'opened';

export async function openCardLink(
	row: string,
	matchId: string,
	episode: number,
	deps: OpenCardLinkDeps
): Promise<CardLinkOpening> {
	if (deps.busy()) return 'held';
	deps.dismissFailure();
	const guessed = await deps.stillGuessed(row, matchId);
	if (guessed === 'unread') {
		deps.checkFailed();
		return 'unread';
	}
	if (!guessed) return 'settled';
	if (deps.busy()) return 'overtaken';
	deps.start(episode);
	return 'opened';
}

/** A guessed card's failed resume: its row, guess and episode, for the
 *  overlay's "Pick the right show". */
export interface FailedGuess {
	row: string;
	matchId: string;
	title: string;
	episode: number;
}

export interface FailedResumeDeps {
	/** The history line `row`, as Home read it. */
	line: (row: string) => { id: string; title: string } | undefined;
	stillGuessed: (row: string, matchId: string) => Promise<CardCheck>;
	/** Whether the click the failure is about is still the viewer's. */
	isCurrent: () => boolean;
}

/** The pick a failed resume offers: for a guess still standing, and
 *  for one whose check could not be read, since the pick checks again;
 *  none for a play no guess opened, or a card settled since the cards
 *  were read. `null` when the viewer moved on meanwhile, so nothing is
 *  shown at all. */
export async function failedResumeGuess(
	args: { guess?: boolean; row?: string; matchId: string; episode: number },
	deps: FailedResumeDeps
): Promise<FailedGuess | undefined | null> {
	const line = args.guess ? deps.line(args.row ?? '') : undefined;
	const standing = line ? await deps.stillGuessed(line.id, args.matchId) : false;
	if (!deps.isCurrent()) return null;
	return line && standing
		? { row: line.id, matchId: args.matchId, title: line.title, episode: args.episode }
		: undefined;
}
