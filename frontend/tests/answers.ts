// The backend's answers as the tests fake them.
//
// A fake answer is typed against the renderer's own type for that
// route, so a fake that drifts from what the backend sends fails
// type-checking instead of passing a test on a shape no backend
// produces. A success answer names its type at the call:
// `HttpResponse.json<KitsuAnimeRef[]>([...])` (MSW reads the type
// argument, and checks the body against it, with `NoInfer`). What is
// here covers the rest:
//
// - `errorAnswer` is an error answer: a status and the error envelope the
//   backend sends with it, or `null` for an answer with no envelope.
// - The builders (`availabilityAnswer`, `appInfoAnswer`,
//   `sessionAnswer`, `kitsuRefAnswer`, `episodeAnswer`, `configAnswer`)
//   build an answer with every field the backend always sends, so a
//   fake names only the ones its test is about; `wire<T>` types a
//   stream event's data, and `malformedAs` a value wrong on purpose.
//   They live in `src/lib/testing/wire-answers.ts`, inside the tree the
//   unit tests may import from, and are re-exported here.
// - `malformedAnswer` is an answer no backend sends, on purpose: the test is
//   about what the renderer does with one. Using it says so.
//
// The ESLint config refuses an `HttpResponse.json` call in test code
// that names no type; `tests/lint/typed-answers.test.ts` lists the
// forms it does not see.

import { HttpResponse, type JsonBodyType } from 'msw';

import type { AniErrorPayload } from '../src/lib/api';

export {
	appInfoAnswer,
	availabilityAnswer,
	configAnswer,
	episodeAnswer,
	kitsuRefAnswer,
	malformedAs,
	sessionAnswer,
	wire
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
