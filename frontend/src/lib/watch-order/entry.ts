/**
 * The viewer's local day, which the backend judges "not aired yet"
 * against: an input to its answers, read from the viewer's clock.
 */

/** The viewer's local calendar day, YYYY-MM-DD, which "Coming …" is judged against. */
export function localDay(now: Date): string {
	const pad = (n: number) => String(n).padStart(2, '0');
	return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}
