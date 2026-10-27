// The fields a tracker write sets, and what is left of a held write to
// send once newer writes for the same show have taken some of them.
// Split from `save-when-allowed.ts`, which re-exports it, so each file
// stays inside the CRAP gate's per-file bar.

/** What a write sets on a tracker's entry. */
export type WriteField = 'status' | 'progress';

/** Every field: a write that covers the whole entry. */
export const ALL_FIELDS: readonly WriteField[] = ['status', 'progress'];

/** A write's view, from inside an attempt, of the fields still its own
 *  to send. */
export interface HeldWrite {
	left: () => readonly WriteField[];
}

/**
 * `body` without the fields a newer write has covered: of the fields
 * the write `declared`, those not `left` are dropped, and anything it
 * sets without declaring — the status a new row is created with — rides
 * along with a declared field that is left, never on its own. `null`
 * when none of the declared fields is left, or nothing is left to send.
 */
export function keepFields<B extends { status?: unknown; progress?: unknown }>(
	body: B,
	declared: readonly WriteField[],
	left: readonly WriteField[]
): B | null {
	if (!declared.some((field) => left.includes(field))) return null;
	const kept = { ...body };
	for (const field of declared) if (!left.includes(field)) delete kept[field];
	return kept.status === undefined && kept.progress === undefined ? null : kept;
}

/** What is left of a held write's `body` to send — {@link keepFields},
 *  except that a completion never goes without its episode: the backend
 *  holds a `completed` to the episode beside it, so a completion whose
 *  progress a newer write covered is dropped with it. */
export function keepWrite<B extends { status?: unknown; progress?: unknown }>(
	body: B,
	declared: readonly WriteField[],
	left: readonly WriteField[]
): B | null {
	const episodeGone = declared.includes('progress') && !left.includes('progress');
	if (body.status === 'completed' && episodeGone) return null;
	return keepFields(body, declared, left);
}

/** Which write, by number, last covered each field of one show. */
export type Coverage = Readonly<Partial<Record<WriteField, number>>>;

/** `covered` once write number `generation` of `fields` has started: it
 *  is the newest to cover each of them. */
export function coverFields(
	covered: Coverage | undefined,
	fields: readonly WriteField[],
	generation: number
): Coverage {
	const next: Partial<Record<WriteField, number>> = { ...covered };
	for (const field of fields) next[field] = generation;
	return next;
}

/** The fields of write number `generation` that no newer write has
 *  covered since: those it is still the newest to cover. */
export function fieldsLeft(
	covered: Coverage | undefined,
	fields: readonly WriteField[],
	generation: number
): readonly WriteField[] {
	return fields.filter((field) => covered?.[field] === generation);
}
