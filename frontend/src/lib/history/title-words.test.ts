// The shared-words rule a search hit must pass to be taken for a
// Continue row's show. The backend's show-id resolve applies the same
// rule; both run the vectors in tests/fixtures/title-words, built from
// AniList ↔ Kitsu pairs of the same shows and from the hits that once
// put Here is Greenwood on Seitokai ni mo Ana wa Aru!'s card.

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import type { KitsuAnimeRef } from '$lib/api';
import { sharesWords } from './title-words';

interface Vector {
	case: string;
	row: string[];
	hit: Pick<KitsuAnimeRef, 'canonical_title' | 'titles' | 'abbreviated_titles' | 'slug'>;
	accept: boolean;
}

const vectors: Vector[] = JSON.parse(
	readFileSync(
		path.resolve(
			path.dirname(fileURLToPath(import.meta.url)),
			'../../../../tests/fixtures/title-words/vectors.json'
		),
		'utf8'
	)
);

describe('sharesWords', () => {
	it.each(vectors.map((v) => [v.case, v] as const))('%s', (_name, v) => {
		expect(sharesWords(v.row, v.hit as KitsuAnimeRef)).toBe(v.accept);
	});

	it('runs every shared vector', () => {
		expect(vectors.length).toBeGreaterThan(80);
	});
});
