/**
 * When the bell rings: the moment its count goes from nothing to
 * something. It swings sideways once and the count pops onto it; later
 * notices only change the number. Under reduced motion it stays still.
 */
export function shouldRing(before: number, after: number, reduced: boolean): boolean {
	return !reduced && before === 0 && after > 0;
}

/** The count as drawn on the bell: at most two digits. */
export function countLabel(count: number): string {
	return count > 99 ? '99+' : String(count);
}
