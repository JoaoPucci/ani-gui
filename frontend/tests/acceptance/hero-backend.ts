// The home hero's routes as a scenario's backend answers them: the pick
// (`POST /api/home/hero`) made by the fake of the backend's pick
// (src/test/hero-pick-fake.ts, held to the backend's golden cases)
// under the airing preference the scenario names, and the Debug page's
// read of the last walk (`GET /api/home/hero/walk`), which answers
// the walk of the last pick this backend made.
//
// setup.ts installs one with the preference off and the defaults'
// origin, so every scenario that mounts Home gets the pick the app
// makes with no flags file. A scenario about the preference installs
// its own, which takes precedence, and reads what Home sent from
// `bodies`.

import { http, HttpResponse, type HttpHandler } from 'msw';

import { API_BASE } from './setup';
import { heroPick } from '../../src/test/hero-pick-fake';

import type { HeroPick, HeroPickBody, HeroWalk, HeroWalkView } from '../../src/lib/api';

export interface HeroBackend {
	handlers: HttpHandler[];
	/** Every body Home sent for a pick, in order. */
	bodies: HeroPickBody[];
}

export function heroBackend({
	airingFirst = false,
	origin = 'defaults'
}: { airingFirst?: boolean; origin?: string } = {}): HeroBackend {
	const bodies: HeroPickBody[] = [];
	let last: HeroWalk | null = null;
	return {
		bodies,
		handlers: [
			http.post(`${API_BASE}/api/home/hero`, async ({ request }) => {
				const body = (await request.json()) as HeroPickBody;
				bodies.push(body);
				const pick = heroPick(body.shows, airingFirst);
				last = pick.walk;
				return HttpResponse.json<HeroPick>(pick);
			}),
			http.get(`${API_BASE}/api/home/hero/walk`, () =>
				HttpResponse.json<HeroWalkView>({
					flag: { name: 'hero_airing_first', on: airingFirst, origin },
					walk: last
				})
			)
		]
	};
}
