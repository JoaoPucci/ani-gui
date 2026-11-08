// Word choices the maintainer settled for the non-English locales.
// Each rule names the messages it governs, or is a plain character
// check over every message of a locale. None of them reads a sentence
// to decide what it means.
//
// - Portuguese and Spanish speak of the user's lists ("suas listas",
//   "tus listas"), never of a "rastreador", for AniList and
//   MyAnimeList.
// - Portuguese and Spanish spell the audio out (legendado / dublado,
//   subtitulado / doblado) instead of the English SUB and DUB, the
//   toggle included.
// - The plan-to-watch status is "Quero assistir" and "Por ver"
//   wherever a message names it.
// - A message that means the show says "anime", not "título". Where
//   "título" means a name (an episode title, a title typed into
//   search), it stays, so the rule lists the show-meaning keys.
// - Russian headings end without a full stop.
// - "Again" is one phrase per locale: "de novo" in Portuguese (never
//   "novamente"), «Попробуйте снова» in Russian (never «Попробуйте ещё
//   раз»).
// - Russian calls the streaming source «Источник стриминга», never
//   «Источник видео».
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { m } from '$lib/paraglide/messages';

type Message = (inputs: Record<string, never>, options: { locale: string }) => string;
const message = (name: string): Message | undefined =>
	(m as unknown as Record<string, Message | undefined>)[name];

const root = join(__dirname, '..', '..', 'messages');

/** Every string in a message tree: plain messages and plural arms. */
function strings(value: unknown): string[] {
	if (typeof value === 'string') return [value];
	if (Array.isArray(value)) return value.flatMap(strings);
	if (value && typeof value === 'object') {
		const v = value as Record<string, unknown>;
		// A plural message's declarations and selectors are syntax, not
		// text: only its match arms are shown.
		if ('match' in v) return strings(v.match);
		return Object.values(v).flatMap(strings);
	}
	return [];
}

/** `namespace.key` → every shown string of that message, for a locale. */
function bundle(locale: string): Map<string, string[]> {
	const out = new Map<string, string[]>();
	for (const file of readdirSync(join(root, locale)).filter((f) => f.endsWith('.json'))) {
		const ns = file.slice(0, -'.json'.length);
		const parsed = JSON.parse(readFileSync(join(root, locale, file), 'utf8'));
		for (const [key, value] of Object.entries(parsed)) {
			if (key === '$schema') continue;
			out.set(`${ns}.${key}`, strings(value));
		}
	}
	return out;
}

const BUNDLES = new Map(['pt-BR', 'es-419', 'ru'].map((l) => [l, bundle(l)]));

function offenders(locale: string, test: (s: string) => boolean, keys?: string[]): string[] {
	const b = BUNDLES.get(locale)!;
	const found: string[] = [];
	for (const key of keys ?? [...b.keys()]) {
		const values = b.get(key);
		expect(values, `${locale} lacks ${key}`).toBeDefined();
		if (values!.some(test)) found.push(key);
	}
	return found;
}

describe('Portuguese and Spanish name the list services as lists', () => {
	for (const locale of ['pt-BR', 'es-419']) {
		it(`no ${locale} message says "rastreador"`, () => {
			expect(offenders(locale, (s) => /rastread/i.test(s))).toEqual([]);
		});
	}
});

describe('Portuguese and Spanish spell the audio out', () => {
	for (const locale of ['pt-BR', 'es-419']) {
		it(`no ${locale} message uses the SUB or DUB acronym`, () => {
			expect(offenders(locale, (s) => /\b(sub|dub)\b/i.test(s))).toEqual([]);
		});
	}

	it('the audio toggle is labelled in each locale', () => {
		// English and Russian keep the short labels the toggle always
		// had, and their prose names the audio the same way.
		const labels = {
			en: ['SUB', 'DUB'],
			'pt-BR': ['Legendado', 'Dublado'],
			'es-419': ['Subtitulado', 'Doblado'],
			ru: ['SUB', 'DUB']
		};
		for (const [locale, [sub, dub]] of Object.entries(labels) as [
			'en' | 'pt-BR' | 'es-419' | 'ru',
			string[]
		][]) {
			expect(message('app_audio_sub')?.({}, { locale }), locale).toBe(sub);
			expect(message('app_audio_dub')?.({}, { locale }), locale).toBe(dub);
		}
	});
});

describe('the plan-to-watch status has one name', () => {
	const PLANNING_KEYS = [
		'detail.list_status_planning',
		'account.watch_later_caption',
		'account.watch_later_empty',
		'account.watch_later_failed_hint'
	];
	const names = { 'pt-BR': 'Quero assistir', 'es-419': 'Por ver' };
	for (const [locale, name] of Object.entries(names)) {
		it(`${locale} calls it "${name}"`, () => {
			expect(offenders(locale, (s) => !s.includes(name), PLANNING_KEYS)).toEqual([]);
		});
	}
});

describe('a message about the show says "anime"', () => {
	/** Keys whose "title" means the show, not a name. */
	const SHOW_KEYS = [
		'account.watch_later_empty_hint',
		'account.primary_hint',
		'backend.error_play_episode_unavailable',
		'backend.error_play_stream_server_failed',
		'backend.error_search_no_results',
		'detail.actions_aria_label',
		'detail.error_load_headline',
		'detail.error_play_no_results',
		'detail.meta_row_aria_label',
		'detail.similar_strip_eyebrow',
		'detail.synopsis_empty',
		'detail.unavailable_message',
		'detail.unavailable_in_dub_message',
		'detail.unavailable_in_sub_message',
		'detail.error_no_id_detail',
		'download.disabled_all_tooltip',
		'download.range_error_over_cap',
		'play.error_no_session',
		'play.error_stream_back_to_show',
		'play.error_stream_unavailable_in_dub',
		'play.error_stream_unavailable_in_sub',
		'play.similar_strip_eyebrow',
		'play.play_failure_no_results',
		'play.play_failure_episode_unavailable',
		'play.play_failure_episode_unavailable_try_other_audio',
		'play.error_no_show_id',
		'play.error_load_failed',
		'settings.field_custom_controls_hint'
	];
	// A whole word: "subtítulos" is not the word.
	for (const locale of ['pt-BR', 'es-419']) {
		it(`${locale} show messages do not call it "título"`, () => {
			expect(offenders(locale, (s) => /(^|[^\p{L}])t[íi]tulo/iu.test(s), SHOW_KEYS)).toEqual([]);
		});
	}
});

describe('Russian headings end without a full stop', () => {
	/** Page titles, section and dialog titles, error and empty-state headlines. */
	const HEADING_KEYS = [
		'about.section_donate_title',
		'about.section_assets_title',
		'about.section_bundled_tools_title',
		'about.section_license_title',
		'account.title',
		'account.disconnect_confirm_title',
		'account.watch_later_title',
		'account.watch_later_empty',
		'account.watch_later_failed',
		'detail.episodes_heading',
		'detail.synopsis_heading',
		'detail.error_load_headline',
		'detail.error_no_id_headline',
		'detail.error_play_headline',
		'detail.error_play_headline_single',
		'download.dock_title',
		'download.error_ffmpeg_missing_headline',
		'home.hero_error_title',
		'home.error_resume_headline',
		'home.delete_confirm_title',
		'home.clear_confirm_title',
		'play.error_stream_headline',
		'play.error_play_headline',
		'play.error_play_headline_single',
		'play.error_external_headline',
		'play.error_external_headline_single',
		'play.error_syncplay_headline',
		'play.error_syncplay_headline_single',
		// The empty-search headline is "prefix «query» suffix".
		'search.empty_headline_suffix',
		'search.filtered_empty_headline',
		'search.error_headline',
		'settings.title',
		'settings.error_headline',
		'settings.error_save_headline',
		'settings.section_playback_title',
		'settings.section_library_title',
		'settings.section_about_title',
		'update.dialog_title'
	];
	it('no ru heading ends with "."', () => {
		expect(offenders('ru', (s) => s.endsWith('.'), HEADING_KEYS)).toEqual([]);
	});
});

describe('one phrase per term', () => {
	it('Portuguese says "de novo", never "novamente"', () => {
		expect(offenders('pt-BR', (s) => /novamente/i.test(s))).toEqual([]);
	});

	it('Russian says «Попробуйте снова», never «Попробуйте ещё раз»', () => {
		expect(offenders('ru', (s) => /Попробуйте е[её] раз/i.test(s))).toEqual([]);
	});

	it('Russian never calls the streaming source «Источник видео»', () => {
		expect(offenders('ru', (s) => /источник\S* видео/i.test(s))).toEqual([]);
	});
});
