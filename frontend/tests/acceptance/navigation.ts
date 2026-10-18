// A navigation starting, as SvelteKit reports one to a page.
//
// Every scenario mocks `$app/navigation`, so `beforeNavigate` is a
// `vi.fn()` that records the callbacks the mounted pages registered.
// Calling them is what SvelteKit does the moment the viewer follows a
// link or presses Back, before the page they are leaving goes. A
// scenario clears the mock before it mounts, so only its own page
// hears the navigation.

import { vi } from 'vitest';
import { beforeNavigate } from '$app/navigation';

/** Report a navigation starting to every page that registered for one. */
export function startNavigation(): void {
	for (const [callback] of vi.mocked(beforeNavigate).mock.calls) {
		(callback as (nav: unknown) => void)({ type: 'link', willUnload: false, cancel() {} });
	}
}

/** Forget the pages earlier scenarios mounted. */
export function forgetNavigationListeners(): void {
	vi.mocked(beforeNavigate).mockClear();
}
