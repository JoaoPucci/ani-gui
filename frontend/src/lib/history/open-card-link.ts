/**
 * Opening Home's picker for a guessed Continue card — from its pill, or
 * its failed resume's "Pick the right show".
 *
 * A remove or Clear history holds its dialog while it is out, with
 * nothing over it ($lib/history/confirm-run.ts), and a resume its
 * loading overlay, so nothing opens then.
 * Otherwise the failure overlay, if any, goes. A card whose line could
 * not be read ($lib/history/card-lines.svelte.ts) says it could not be
 * checked, as a failed check does. Otherwise the card is checked
 * again ($lib/history/card-relink.ts): the picker opens only on a card
 * still guessed, and only if no remove, clear or resume began while the
 * check was out. A check that could not be read says so, and leaves the pill
 * to try again; a card settled since the cards were read, or matched
 * again since its check began, opens nothing.
 *
 * Home is an adapter: it hands in the busy flags, the overlay, the
 * check, the notice and the picker.
 */

/** What the check answered: still guessed, no longer, or unknown. */
export type CardCheck = boolean | 'unread';

export interface OpenCardLinkDeps {
	/** Whether a remove, a Clear history or a resume is out. */
	busy: () => boolean;
	/** Take down the failure overlay. */
	dismissFailure: () => void;
	stillGuessed: (row: string, matchId: string) => Promise<CardCheck>;
	/** Say the card could not be checked. */
	checkFailed: () => void;
	/** Open the picker, naming `episode`. */
	start: (episode: number) => void;
}

/** What came of the opening; `settled` covers a card matched again
 *  since its check began too. */
export type CardLinkOpening = 'held' | 'unread' | 'settled' | 'overtaken' | 'opened';

export async function openCardLink(
	row: string | null,
	matchId: string,
	episode: number,
	deps: OpenCardLinkDeps
): Promise<CardLinkOpening> {
	if (deps.busy()) return 'held';
	deps.dismissFailure();
	// The card's line could not be read: it is not known which line the
	// pill stands for, and the card's own row may not be it.
	if (row === null) {
		deps.checkFailed();
		return 'unread';
	}
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
 *  none for a play no guess opened, one whose card line could not be
 *  read (`unread`: the row it played may not be the line), or a card
 *  settled since the cards were read or matched again since the check
 *  began. `null` when the viewer moved on meanwhile, so nothing is
 *  shown at all. */
export async function failedResumeGuess(
	args: { guess?: boolean; row?: string; matchId: string; episode: number; unread?: boolean },
	deps: FailedResumeDeps
): Promise<FailedGuess | undefined | null> {
	const line = args.guess && !args.unread ? deps.line(args.row ?? '') : undefined;
	const standing = line ? await deps.stillGuessed(line.id, args.matchId) : false;
	if (!deps.isCurrent()) return null;
	return line && standing
		? { row: line.id, matchId: args.matchId, title: line.title, episode: args.episode }
		: undefined;
}
