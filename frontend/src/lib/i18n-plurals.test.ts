// Every message that puts a word after a count inflects that word for
// the count, through Paraglide's plural variants (Intl.PluralRules).
//
// Russian has three integer forms — one (1, 21, 31…), few (2–4,
// 22–24…) and many (5–20, 25–30…) — so it is pinned on numbers
// that land in each, including the teens that look like 1 and 2 but
// are not. English, Brazilian Portuguese and Latin American Spanish
// are pinned on 0, 1 and 2: zero takes the plural in all three, which
// CLDR's Portuguese rule does not give on its own (it files 0 under
// "one"), so the Portuguese messages match 0 explicitly.
//
// Messages are looked up by name so the table can name a message the
// catalogue does not have yet and still type-check; a missing one
// fails its row rather than the compile.
import { describe, expect, it } from 'vitest';
import { m } from '$lib/paraglide/messages';

type Locale = 'en' | 'pt-BR' | 'es-419' | 'ru';
type Message = (inputs: Record<string, unknown>, options: { locale: Locale }) => string;

/** Russian: one (1, 21), few (2, 22), many (5, 25) and a teen (11). */
type RuCount = 1 | 2 | 5 | 21 | 22 | 25 | 11;
/** English, Portuguese, Spanish: zero, one and two. */
type LatinCount = 0 | 1 | 2;

type Row = {
	message: string;
	/** Builds the message's inputs from the count under test. */
	inputs?: (n: number) => Record<string, unknown>;
	en: Record<LatinCount, string>;
	'pt-BR': Record<LatinCount, string>;
	'es-419': Record<LatinCount, string>;
	ru: Record<RuCount, string>;
};

const count = (n: number) => ({ count: n });

/** What the dock's speed label carries beside its count. */
const SPEED = '1.2 MB/s';

const ROWS: Row[] = [
	{
		message: 'search_count_word',
		en: { 0: 'results', 1: 'result', 2: 'results' },
		'pt-BR': { 0: 'resultados', 1: 'resultado', 2: 'resultados' },
		'es-419': { 0: 'resultados', 1: 'resultado', 2: 'resultados' },
		ru: {
			1: 'результат',
			2: 'результата',
			5: 'результатов',
			21: 'результат',
			22: 'результата',
			25: 'результатов',
			11: 'результатов'
		}
	},
	{
		message: 'search_card_eps_suffix',
		en: { 0: 'eps', 1: 'ep', 2: 'eps' },
		'pt-BR': { 0: 'eps', 1: 'ep', 2: 'eps' },
		'es-419': { 0: 'eps', 1: 'ep', 2: 'eps' },
		// The abbreviation does not inflect.
		ru: { 1: 'эп.', 2: 'эп.', 5: 'эп.', 21: 'эп.', 22: 'эп.', 25: 'эп.', 11: 'эп.' }
	},
	...['detail_episodes_range_all', 'play_episodes_range_all'].map((message) => ({
		message,
		en: { 0: '0 episodes', 1: '1 episode', 2: '2 episodes' },
		'pt-BR': { 0: '0 episódios', 1: '1 episódio', 2: '2 episódios' },
		'es-419': { 0: '0 episodios', 1: '1 episodio', 2: '2 episodios' },
		ru: {
			1: '1 эпизод',
			2: '2 эпизода',
			5: '5 эпизодов',
			21: '21 эпизод',
			22: '22 эпизода',
			25: '25 эпизодов',
			11: '11 эпизодов'
		}
	})),
	{
		message: 'detail_episodes_range_aired',
		en: { 0: '0 aired', 1: '1 aired', 2: '2 aired' },
		'pt-BR': { 0: '0 exibidos', 1: '1 exibido', 2: '2 exibidos' },
		'es-419': { 0: '0 emitidos', 1: '1 emitido', 2: '2 emitidos' },
		ru: {
			1: '1 вышел',
			2: '2 вышло',
			5: '5 вышло',
			21: '21 вышел',
			22: '22 вышло',
			25: '25 вышло',
			11: '11 вышло'
		}
	},
	{
		message: 'detail_episodes_range_aired_so_far',
		en: { 0: '0 aired so far', 1: '1 aired so far', 2: '2 aired so far' },
		'pt-BR': { 0: '0 exibidos até agora', 1: '1 exibido até agora', 2: '2 exibidos até agora' },
		'es-419': {
			0: '0 emitidos hasta ahora',
			1: '1 emitido hasta ahora',
			2: '2 emitidos hasta ahora'
		},
		ru: {
			1: '1 вышел',
			2: '2 вышло',
			5: '5 вышло',
			21: '21 вышел',
			22: '22 вышло',
			25: '25 вышло',
			11: '11 вышло'
		}
	},
	...['detail_episodes_range_announced_suffix', 'download_range_announced_suffix'].map(
		(message) => ({
			message,
			en: { 0: ' · 0 announced', 1: ' · 1 announced', 2: ' · 2 announced' },
			'pt-BR': { 0: ' · 0 anunciados', 1: ' · 1 anunciado', 2: ' · 2 anunciados' },
			'es-419': { 0: ' · 0 anunciados', 1: ' · 1 anunciado', 2: ' · 2 anunciados' },
			ru: {
				1: ' · 1 анонсирован',
				2: ' · 2 анонсировано',
				5: ' · 5 анонсировано',
				21: ' · 21 анонсирован',
				22: ' · 22 анонсировано',
				25: ' · 25 анонсировано',
				11: ' · 11 анонсировано'
			}
		})
	),
	{
		message: 'detail_episodes_range_expected_suffix',
		en: { 0: ' · 0 expected', 1: ' · 1 expected', 2: ' · 2 expected' },
		'pt-BR': { 0: ' · 0 previstos', 1: ' · 1 previsto', 2: ' · 2 previstos' },
		'es-419': { 0: ' · 0 previstos', 1: ' · 1 previsto', 2: ' · 2 previstos' },
		// "ожидается" is impersonal: it reads right after any number.
		ru: {
			1: ' · 1 ожидается',
			2: ' · 2 ожидается',
			5: ' · 5 ожидается',
			21: ' · 21 ожидается',
			22: ' · 22 ожидается',
			25: ' · 25 ожидается',
			11: ' · 11 ожидается'
		}
	},
	{
		message: 'home_clear_confirm_title',
		en: {
			0: 'Remove 0 entries from history?',
			1: 'Remove 1 entry from history?',
			2: 'Remove 2 entries from history?'
		},
		'pt-BR': {
			0: 'Remover 0 entradas do histórico?',
			1: 'Remover 1 entrada do histórico?',
			2: 'Remover 2 entradas do histórico?'
		},
		'es-419': {
			0: '¿Quitar 0 entradas del historial?',
			1: '¿Quitar 1 entrada del historial?',
			2: '¿Quitar 2 entradas del historial?'
		},
		ru: {
			1: 'Удалить 1 запись из истории?',
			2: 'Удалить 2 записи из истории?',
			5: 'Удалить 5 записей из истории?',
			21: 'Удалить 21 запись из истории?',
			22: 'Удалить 22 записи из истории?',
			25: 'Удалить 25 записей из истории?',
			11: 'Удалить 11 записей из истории?'
		}
	},
	{
		message: 'download_eyebrow_value_all',
		// One episode is not "all 1 episodes": the single case drops "all".
		en: { 0: 'all 0 episodes', 1: '1 episode', 2: 'all 2 episodes' },
		'pt-BR': { 0: 'todos os 0 episódios', 1: '1 episódio', 2: 'todos os 2 episódios' },
		'es-419': { 0: 'los 0 episodios', 1: '1 episodio', 2: 'los 2 episodios' },
		ru: {
			1: '1 эпизод',
			2: 'все 2 эпизода',
			5: 'все 5 эпизодов',
			21: 'все 21 эпизод',
			22: 'все 22 эпизода',
			25: 'все 25 эпизодов',
			11: 'все 11 эпизодов'
		}
	},
	{
		message: 'download_hint_all_total',
		en: {
			0: 'Downloads all 0 episodes sequentially',
			1: 'Downloads 1 episode',
			2: 'Downloads all 2 episodes sequentially'
		},
		'pt-BR': {
			0: 'Baixa todos os 0 episódios em sequência',
			1: 'Baixa 1 episódio',
			2: 'Baixa todos os 2 episódios em sequência'
		},
		'es-419': {
			0: 'Descarga los 0 episodios secuencialmente',
			1: 'Descarga 1 episodio',
			2: 'Descarga los 2 episodios secuencialmente'
		},
		ru: {
			1: 'Скачает 1 эпизод',
			2: 'Скачает все 2 эпизода подряд',
			5: 'Скачает все 5 эпизодов подряд',
			21: 'Скачает все 21 эпизод подряд',
			22: 'Скачает все 22 эпизода подряд',
			25: 'Скачает все 25 эпизодов подряд',
			11: 'Скачает все 11 эпизодов подряд'
		}
	},
	{
		message: 'download_hint_all_total_aired',
		en: {
			0: 'Downloads all 0 aired episodes sequentially',
			1: 'Downloads 1 aired episode',
			2: 'Downloads all 2 aired episodes sequentially'
		},
		'pt-BR': {
			0: 'Baixa todos os 0 episódios exibidos em sequência',
			1: 'Baixa 1 episódio exibido',
			2: 'Baixa todos os 2 episódios exibidos em sequência'
		},
		'es-419': {
			0: 'Descarga los 0 episodios emitidos secuencialmente',
			1: 'Descarga 1 episodio emitido',
			2: 'Descarga los 2 episodios emitidos secuencialmente'
		},
		ru: {
			1: 'Скачает 1 вышедший эпизод',
			2: 'Скачает все 2 вышедших эпизода подряд',
			5: 'Скачает все 5 вышедших эпизодов подряд',
			21: 'Скачает все 21 вышедший эпизод подряд',
			22: 'Скачает все 22 вышедших эпизода подряд',
			25: 'Скачает все 25 вышедших эпизодов подряд',
			11: 'Скачает все 11 вышедших эпизодов подряд'
		}
	},
	{
		message: 'download_hint_all_gap_suffix',
		// The gap only shows while fewer have been released than were
		// announced; the count under test is the released one.
		inputs: (n) => ({ announced: 30, available: n }),
		en: {
			0: ' — 30 are announced but only 0 have been released so far.',
			1: ' — 30 are announced but only 1 has been released so far.',
			2: ' — 30 are announced but only 2 have been released so far.'
		},
		'pt-BR': {
			0: ' — 30 foram anunciados, mas só 0 foram lançados até agora.',
			1: ' — 30 foram anunciados, mas só 1 foi lançado até agora.',
			2: ' — 30 foram anunciados, mas só 2 foram lançados até agora.'
		},
		'es-419': {
			0: ' — 30 están anunciados pero solo 0 se han estrenado hasta ahora.',
			1: ' — 30 están anunciados pero solo 1 se ha estrenado hasta ahora.',
			2: ' — 30 están anunciados pero solo 2 se han estrenado hasta ahora.'
		},
		ru: {
			1: ' — анонсировано 30, но вышел пока только 1.',
			2: ' — анонсировано 30, но вышло пока только 2.',
			5: ' — анонсировано 30, но вышло пока только 5.',
			21: ' — анонсировано 30, но вышел пока только 21.',
			22: ' — анонсировано 30, но вышло пока только 22.',
			25: ' — анонсировано 30, но вышло пока только 25.',
			11: ' — анонсировано 30, но вышло пока только 11.'
		}
	},
	{
		message: 'download_hint_range',
		en: {
			0: 'Downloads 0 episodes sequentially.',
			1: 'Downloads 1 episode.',
			2: 'Downloads 2 episodes sequentially.'
		},
		'pt-BR': {
			0: 'Baixa 0 episódios em sequência.',
			1: 'Baixa 1 episódio.',
			2: 'Baixa 2 episódios em sequência.'
		},
		'es-419': {
			0: 'Descarga 0 episodios secuencialmente.',
			1: 'Descarga 1 episodio.',
			2: 'Descarga 2 episodios secuencialmente.'
		},
		ru: {
			1: 'Скачает 1 эпизод.',
			2: 'Скачает 2 эпизода подряд.',
			5: 'Скачает 5 эпизодов подряд.',
			21: 'Скачает 21 эпизод подряд.',
			22: 'Скачает 22 эпизода подряд.',
			25: 'Скачает 25 эпизодов подряд.',
			11: 'Скачает 11 эпизодов подряд.'
		}
	},
	{
		message: 'download_bar_label_multi',
		en: { 0: 'Downloading 0 episodes', 1: 'Downloading 1 episode', 2: 'Downloading 2 episodes' },
		'pt-BR': { 0: 'Baixando 0 episódios', 1: 'Baixando 1 episódio', 2: 'Baixando 2 episódios' },
		'es-419': {
			0: 'Descargando 0 episodios',
			1: 'Descargando 1 episodio',
			2: 'Descargando 2 episodios'
		},
		// After "Загрузка" the count is genitive: 1 серии, 2 / 5 серий.
		ru: {
			1: 'Загрузка 1 серии',
			2: 'Загрузка 2 серий',
			5: 'Загрузка 5 серий',
			21: 'Загрузка 21 серии',
			22: 'Загрузка 22 серий',
			25: 'Загрузка 25 серий',
			11: 'Загрузка 11 серий'
		}
	},
	{
		message: 'download_bar_cancel_confirm_multi',
		en: {
			0: 'Cancel 0 active downloads?',
			1: 'Cancel 1 active download?',
			2: 'Cancel 2 active downloads?'
		},
		'pt-BR': {
			0: 'Cancelar 0 downloads ativos?',
			1: 'Cancelar 1 download ativo?',
			2: 'Cancelar 2 downloads ativos?'
		},
		'es-419': {
			0: '¿Cancelar 0 descargas activas?',
			1: '¿Cancelar 1 descarga activa?',
			2: '¿Cancelar 2 descargas activas?'
		},
		ru: {
			1: 'Отменить 1 активную загрузку?',
			2: 'Отменить 2 активные загрузки?',
			5: 'Отменить 5 активных загрузок?',
			21: 'Отменить 21 активную загрузку?',
			22: 'Отменить 22 активные загрузки?',
			25: 'Отменить 25 активных загрузок?',
			11: 'Отменить 11 активных загрузок?'
		}
	},
	{
		message: 'download_dock_active_speed_label',
		inputs: (n) => ({ count: n, speed: SPEED }),
		en: {
			0: '0 downloads in progress, ' + SPEED,
			1: '1 download in progress, ' + SPEED,
			2: '2 downloads in progress, ' + SPEED
		},
		'pt-BR': {
			0: '0 downloads em andamento, ' + SPEED,
			1: '1 download em andamento, ' + SPEED,
			2: '2 downloads em andamento, ' + SPEED
		},
		'es-419': {
			0: '0 descargas en curso, ' + SPEED,
			1: '1 descarga en curso, ' + SPEED,
			2: '2 descargas en curso, ' + SPEED
		},
		ru: {
			1: '1 загрузка в процессе, ' + SPEED,
			2: '2 загрузки в процессе, ' + SPEED,
			5: '5 загрузок в процессе, ' + SPEED,
			21: '21 загрузка в процессе, ' + SPEED,
			22: '22 загрузки в процессе, ' + SPEED,
			25: '25 загрузок в процессе, ' + SPEED,
			11: '11 загрузок в процессе, ' + SPEED
		}
	},
	{
		message: 'download_dock_unseen_label',
		en: { 0: '0 downloads finished', 1: '1 download finished', 2: '2 downloads finished' },
		'pt-BR': {
			0: '0 downloads concluídos',
			1: '1 download concluído',
			2: '2 downloads concluídos'
		},
		'es-419': {
			0: '0 descargas finalizadas',
			1: '1 descarga finalizada',
			2: '2 descargas finalizadas'
		},
		ru: {
			1: '1 загрузка завершена',
			2: '2 загрузки завершены',
			5: '5 загрузок завершено',
			21: '21 загрузка завершена',
			22: '22 загрузки завершены',
			25: '25 загрузок завершено',
			11: '11 загрузок завершено'
		}
	},
	{
		message: 'download_range_error_over_cap',
		inputs: (n) => ({ max: n }),
		en: {
			0: "You can't download more than 0 episodes for this show.",
			1: "You can't download more than 1 episode for this show.",
			2: "You can't download more than 2 episodes for this show."
		},
		'pt-BR': {
			0: 'Não é possível baixar mais de 0 episódios deste título.',
			1: 'Não é possível baixar mais de 1 episódio deste título.',
			2: 'Não é possível baixar mais de 2 episódios deste título.'
		},
		'es-419': {
			0: 'No puedes descargar más de 0 episodios de este título.',
			1: 'No puedes descargar más de 1 episodio de este título.',
			2: 'No puedes descargar más de 2 episodios de este título.'
		},
		// After "больше" the count is genitive: 1 эпизода, 2 / 5 эпизодов.
		ru: {
			1: 'Нельзя загрузить больше 1 эпизода этого тайтла.',
			2: 'Нельзя загрузить больше 2 эпизодов этого тайтла.',
			5: 'Нельзя загрузить больше 5 эпизодов этого тайтла.',
			21: 'Нельзя загрузить больше 21 эпизода этого тайтла.',
			22: 'Нельзя загрузить больше 22 эпизодов этого тайтла.',
			25: 'Нельзя загрузить больше 25 эпизодов этого тайтла.',
			11: 'Нельзя загрузить больше 11 эпизодов этого тайтла.'
		}
	}
];

const lookup = (name: string): Message | undefined =>
	(m as unknown as Record<string, Message | undefined>)[name];

describe('counted words agree with their count', () => {
	for (const row of ROWS) {
		const inputs = row.inputs ?? count;
		for (const locale of ['en', 'pt-BR', 'es-419', 'ru'] as const) {
			it(`${row.message} in ${locale}`, () => {
				const message = lookup(row.message);
				expect(message, `m.${row.message} exists`).toBeTypeOf('function');
				const expected = row[locale] as Record<number, string>;
				for (const [n, text] of Object.entries(expected)) {
					expect(message!(inputs(Number(n)), { locale }), `${locale} ${n}`).toBe(text);
				}
			});
		}
	}

	// Counts reach some messages as strings (`String(detail.episode_count)`);
	// the plural selector has to read those the same as numbers.
	it('a count passed as a string selects the same form', () => {
		const message = lookup('detail_episodes_range_all')!;
		expect(message({ count: '22' }, { locale: 'ru' })).toBe('22 эпизода');
		expect(message({ count: '1' }, { locale: 'en' })).toBe('1 episode');
		expect(message({ count: '0' }, { locale: 'pt-BR' })).toBe('0 episódios');
	});

	// The catalogue no longer carries the either-or pairs and hedges that
	// stood in for plural forms.
	it('the stand-ins for plural forms are gone', () => {
		for (const name of [
			'search_count_singular',
			'search_count_plural',
			'home_clear_confirm_title_one',
			'home_clear_confirm_title_other',
			'download_hint_all_prefix_total',
			'download_hint_all_aired',
			'download_hint_all_episodes_suffix'
		]) {
			expect(lookup(name), name).toBeUndefined();
		}
		for (const locale of ['en', 'pt-BR', 'es-419'] as const) {
			for (const name of ['download_dock_active_speed_label', 'download_dock_unseen_label']) {
				const text = lookup(name)?.({ count: 3, speed: SPEED }, { locale });
				expect(text, `${locale} ${name}`).not.toMatch(/\(s\)/);
			}
		}
	});
});
