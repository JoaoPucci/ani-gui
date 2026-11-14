// The backend's answers as the tests fake them.
//
// A fake answer is typed against the renderer's own type for that
// route, so a fake that drifts from what the backend sends fails
// type-checking instead of passing a test on a shape no backend
// produces. A success answer names its type at the call:
// `HttpResponse.json<KitsuAnimeRef[]>([...])` (MSW reads the type
// argument, and checks the body against it, with `NoInfer`). The two
// helpers here cover the rest:
//
// - `errorAnswer` is an error answer: a status and the error envelope the
//   backend sends with it, or `null` for an answer with no envelope.
// - `appInfoAnswer` and `sessionAnswer` build an answer with every
//   field the backend always sends, so a fake names only the ones its
//   test is about; the builders the unit tests share
//   (`availabilityAnswer`, `kitsuRefAnswer`, `episodeAnswer`,
//   `configAnswer`, and `malformedAs` for a value wrong on purpose) live
//   in `src/lib/testing/wire-answers.ts`, inside the tree the unit tests
//   may import from, and are re-exported here. A field a builder fills
//   is the value the backend sends when it has nothing to say (`null`,
//   empty, `false`).
// - `malformedAnswer` is an answer no backend sends, on purpose: the test is
//   about what the renderer does with one. Using it says so.
//
// The ESLint config refuses an `HttpResponse.json` call without a type
// argument in test code, so a new fake cannot be left untyped.

import { HttpResponse, type JsonBodyType } from 'msw';

import type { AniErrorPayload, AppInfo, CreateSessionResponse } from '../src/lib/api';

export {
	availabilityAnswer,
	configAnswer,
	episodeAnswer,
	kitsuRefAnswer,
	malformedAs
} from '../src/lib/testing/wire-answers';

/** An error answer: `status`, with `envelope` as its body (`null` for
 *  one with no envelope). It fits any handler, whatever the route's
 *  success type. */
export function errorAnswer(
	status: number,
	envelope: AniErrorPayload | null = null
): HttpResponse<never> {
	return HttpResponse.json<AniErrorPayload | null>(envelope, { status }) as HttpResponse<never>;
}

/** An answer no backend sends, deliberately: its body is not the
 *  route's type. */
export function malformedAnswer(body: unknown, init?: ResponseInit): HttpResponse<never> {
	// The one answer typed as any body, by design: see the ESLint rule.
	// eslint-disable-next-line no-restricted-syntax
	return HttpResponse.json<JsonBodyType>(body as JsonBodyType, init) as HttpResponse<never>;
}

/** `/api/app-info` as the backend sends it on a launch whose sweeps
 *  found nothing, with `fields` over it. */
export function appInfoAnswer(fields: Partial<AppInfo> = {}): AppInfo {
	return {
		version: '0.0.0-test',
		history_path: '/tmp/history',
		proxy_base_url: 'http://127.0.0.1:0',
		removed_legacy_paths: [],
		retired_provider_sweep: { superseded: [], renumbered: [], numbering_rows: 0, cache_rows: 0 },
		cache_reclaim: { undeclared_rows: 0, expired_rows: 0 },
		...fields
	};
}

/** A play's session as the backend sends it: a fresh resolve with no
 *  sidecar subtitles, with `fields` over it. */
export function sessionAnswer(
	fields: Pick<CreateSessionResponse, 'session_id' | 'media_url' | 'media_kind'> &
		Partial<CreateSessionResponse>
): CreateSessionResponse {
	return { cache_hit: false, subtitles: [], ...fields };
}
