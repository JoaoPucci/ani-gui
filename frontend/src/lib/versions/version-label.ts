import type { VersionKind } from '$lib/api';
import { m } from '$lib/paraglide/messages';

/** What the version control calls a version. */
export function versionLabel(kind: VersionKind): string {
	return kind === 'uncensored' ? m.detail_version_uncensored() : m.detail_version_standard();
}
