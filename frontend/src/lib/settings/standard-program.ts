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
	void ask;
	void show;
	return () => {};
}

/** What the hint under the field says. */
export type StandardHint =
	| { variant: 'custom' }
	| { variant: 'unsupported' }
	| { variant: 'found'; path: string }
	| { variant: 'missing' }
	| { variant: 'unknown'; program: string };

/** The hint for `kind`, given the answer the page holds. */
export function standardHint(kind: ExternalPlayerKind, found: Found | null): StandardHint {
	void kind;
	void found;
	return { variant: 'custom' };
}

/** The field's greyed placeholder for `kind`. */
export function standardPlaceholder(kind: ExternalPlayerKind, found: Found | null): string {
	void kind;
	void found;
	return '';
}
