/**
 * What the search page does when its address changes: search for the
 * query it names, or stay as it is.
 */

export type AddressStep = 'search' | 'clear' | 'stay';

/** `q` is the address's query, trimmed; `submitted` the query whose
 *  results are on screen. */
export function followAddress(q: string, submitted: string): AddressStep {
	return q && q !== submitted ? 'search' : 'stay';
}
