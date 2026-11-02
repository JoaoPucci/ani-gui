/**
 * The meta line under a mockup card. Signatures only until the copy
 * lands.
 */
import type { RailKey } from './order';
import type { MockEntry, MockRun } from './types';

export interface MetaSegment {
	readonly text: string;
	readonly kind: 'num' | 'word' | 'watched' | 'unit';
}

export function metaLine(run: MockRun, entry: MockEntry, today: string): MetaSegment[] {
	void run;
	void entry;
	void today;
	return [];
}

export function numeral(position: number): string {
	return String(position);
}

export function railKeyText(
	key: RailKey,
	here: boolean,
	label: (e: MockEntry) => string
): string | null {
	void key;
	void here;
	void label;
	return null;
}
