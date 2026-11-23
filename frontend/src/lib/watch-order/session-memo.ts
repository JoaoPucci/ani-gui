/**
 * The last watch-order answer per show, for the rest of the session,
 * so a show page visited again draws its strip in its first frame
 * instead of a few frames after its title. The page still reads the
 * answer again and draws what it says.
 */
import type { ShowView } from '$lib/api';
import type { WatchOrderAnswer } from './loader';

/** The show page's answer: its view, or none when it could not be read. */
export type ShowAnswer = WatchOrderAnswer<ShowView>;

/** The most shows remembered; the least recently used goes first. */
export const MEMO_LIMIT = 200;

/** In use order: the least recently used first. */
const answers = new Map<string, ShowAnswer>();

function keep(id: string, data: ShowAnswer): void {
	answers.delete(id);
	answers.set(id, data);
	if (answers.size > MEMO_LIMIT) answers.delete(answers.keys().next().value!);
}

/** The answer remembered for `id`, or null. */
export function rememberedAnswer(id: string): ShowAnswer | null {
	const data = answers.get(id);
	if (data === undefined) return null;
	keep(id, data);
	return data;
}

/**
 * What to draw for `id` now that `fresh` has answered, remembering it:
 * `fresh`, unless it could not be read and an earlier answer could,
 * which then stays (a failed read does not take a strip away).
 */
export function settledAnswer(id: string, fresh: ShowAnswer): ShowAnswer {
	const earlier = answers.get(id);
	const shown = fresh.view === null && earlier !== undefined ? earlier : fresh;
	if (shown.view !== null) keep(id, shown);
	return shown;
}

/**
 * What a show page's visit draws for `id` once its read has answered:
 * `settledAnswer`, or null for a visit that has ended (`signal`
 * aborted, or no answer at all), which draws nothing and leaves the
 * remembered answer as it was, since its read was cut off rather than
 * answered.
 */
export function visitAnswer(
	id: string,
	fresh: ShowAnswer | null,
	signal: AbortSignal
): ShowAnswer | null {
	return fresh === null || signal.aborted ? null : settledAnswer(id, fresh);
}

/** Forgets every answer. */
export function forgetAnswers(): void {
	answers.clear();
}

/**
 * The order now in force once `saved` has been saved over `before`.
 * Not yet in place: it forgets nothing.
 */
export function watchOrderSaved(before: string | null, saved: string): string {
	void before;
	return saved;
}
