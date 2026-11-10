/**
 * Whether the page behind is a search the reader ran — the one page
 * behind that the breadcrumb names and steps back to
 * ($lib/breadcrumb's withSearchOrigin, and the layout's back target).
 * Pure; kept apart from the breadcrumb store so the address rules are
 * tested on their own.
 */

/** The base an address of the page behind is parsed against. Any
 *  http(s) origin works: only its being the same for every address
 *  matters. */
const APP_BASE = 'http://app.invalid';

/** `origin` when it is a search the reader ran (the search page with a
 *  query on it), else null: the one page behind that the breadcrumb
 *  names and steps back to.
 *
 *  The layout records the page behind as the pathname and query of the
 *  URL it was on ($lib/history/nav-depth), so a search the reader ran
 *  is written exactly that way: `origin` is answered only when parsing
 *  it on this app gives back the same pathname and query. Anything else
 *  — an address the URL parser refuses, another host or scheme, a
 *  relative or unnormalised path, a fragment — is not that page, and is
 *  never linked as it. Total: answers null rather than throwing. */
export function searchOrigin(origin: string | null): string | null {
	if (!origin) return null;
	let url: URL;
	try {
		url = new URL(origin, APP_BASE);
	} catch {
		return null; // not an address the page behind can have
	}
	if (url.origin !== APP_BASE || url.pathname + url.search !== origin) return null;
	const ran = url.pathname === '/search' && (url.searchParams.get('q') ?? '').trim() !== '';
	return ran ? origin : null;
}
