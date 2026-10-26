// Whether a value is a session as `CreateSessionResponse` declares it
// (src/lib/api.ts), held to the acceptance guard's check of a play's
// answer. The kinds are api.ts's own lists, not copies of them.

import { MEDIA_KINDS, VERSION_KINDS } from '../../src/lib/api';

type Fields = Record<string, unknown>;

const isObject = (v: unknown): v is Fields => typeof v === 'object' && v !== null;
const isString = (v: unknown) => typeof v === 'string';
const optional = (v: unknown, is: (v: unknown) => boolean) => v === undefined || is(v);
const oneOf =
	(kinds: readonly string[]) =>
	(v: unknown): boolean =>
		typeof v === 'string' && kinds.includes(v);
const listOf =
	(is: (v: unknown) => boolean) =>
	(v: unknown): boolean =>
		Array.isArray(v) && v.every(is);

const isSubtitle = (v: unknown) =>
	isObject(v) &&
	isString(v.lang) &&
	isString(v.label) &&
	typeof v.default === 'boolean' &&
	isString(v.url);

const isVersion = (v: unknown) =>
	isObject(v) &&
	oneOf(VERSION_KINDS)(v.kind) &&
	isString(v.show_key) &&
	optional(v.has_mode, (m) => typeof m === 'boolean');

/** Every field the contract requires, with its type, and every
 *  optional one that is present with its declared type. */
export function isSession(v: unknown): boolean {
	return (
		isObject(v) &&
		isString(v.session_id) &&
		isString(v.media_url) &&
		oneOf(MEDIA_KINDS)(v.media_kind) &&
		optional(v.cache_hit, (c) => typeof c === 'boolean') &&
		optional(v.subtitles, listOf(isSubtitle)) &&
		optional(v.episode, isString) &&
		optional(v.former_episode, isString) &&
		optional(v.versions, listOf(isVersion)) &&
		optional(v.version, oneOf(VERSION_KINDS))
	);
}
