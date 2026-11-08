/**
 * What the search page does when its address changes: search for the
 * query it names, clear the search on screen when it names none (the
 * rail's Search link, or a back press onto the bare search page), or
 * stay as it is.
 */

export type AddressStep = 'search' | 'clear' | 'stay';

/** `q` is the address's query, trimmed; `submitted` the query whose
 *  results are on screen. */
export function followAddress(q: string, submitted: string): AddressStep {
	if (q === submitted) return 'stay';
	return q ? 'search' : 'clear';
}
