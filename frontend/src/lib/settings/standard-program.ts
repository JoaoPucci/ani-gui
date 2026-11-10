/**
 * Where the External player type's standard program is installed, as
 * Settings shows it under the field.
 *
 * The page asks the backend once per type ([`createStandardLookup`])
 * and shows only the answer for the type last asked about: one that
 * arrives after the type changed is dropped, and so is a failure. What
 * the hint then says, and the field's placeholder, follow from the
 * type and that answer ([`standardHint`], [`standardPlaceholder`]).
 * The component only passes the type in and renders the result.
 */

import type { ExternalPlayerKind, StandardProgram } from '$lib/api';
import { bareCommand } from './player-command';

/** The backend's answer for a type, as the page keeps it. */
export interface Found {
	kind: ExternalPlayerKind;
	path: string | null;
	supported: boolean;
}

/**
 * A lookup that asks `ask` for a type and calls `show` with its answer,
 * or `null` when asking failed, for the type last passed in only.
 * Custom has no standard program and is never asked about.
 */
export function createStandardLookup(
	ask: (kind: ExternalPlayerKind) => Promise<StandardProgram>,
	show: (found: Found | null) => void
): (kind: ExternalPlayerKind | null | undefined) => void {
	let latest: ExternalPlayerKind | null = null;
	return (kind) => {
		latest = kind ?? null;
		if (!kind || kind === 'custom') return;
		ask(kind).then(
			(answer) => {
				if (latest === kind)
					show({ kind, path: answer.path, supported: answer.supported !== false });
			},
			() => {
				if (latest === kind) show(null);
			}
		);
	};
}

/** What the hint under the field says. */
export type StandardHint =
	| { variant: 'custom' }
	| { variant: 'unsupported' }
	| { variant: 'found'; path: string }
	| { variant: 'missing' }
	| { variant: 'unknown'; program: string };

/** The answer the page holds, if it is for `kind`. */
function answerFor(kind: ExternalPlayerKind, found: Found | null): Found | null {
	return found && found.kind === kind ? found : null;
}

/** The hint for `kind`, given the answer the page holds. */
export function standardHint(kind: ExternalPlayerKind, found: Found | null): StandardHint {
	const program = bareCommand(kind);
	if (program === null) return { variant: 'custom' };
	const answer = answerFor(kind, found);
	if (!answer) return { variant: 'unknown', program };
	if (!answer.supported) return { variant: 'unsupported' };
	return answer.path ? { variant: 'found', path: answer.path } : { variant: 'missing' };
}

/** The field's greyed placeholder for `kind`: the found path, else the
 *  program's name, and nothing for Custom. */
export function standardPlaceholder(kind: ExternalPlayerKind, found: Found | null): string {
	const answer = answerFor(kind, found);
	return (answer?.supported ? answer.path : null) ?? bareCommand(kind) ?? '';
}
