"use strict";

// The main process's own user-visible text.
//
// Nearly everything the user reads is the renderer's and goes through
// Paraglide. The main process shows two dialogs itself, at moments
// the renderer cannot: a boot that failed before a page existed, and
// the prompt on a quit with downloads running, which has to be
// synchronous for the close to be preventable. It cannot reach the
// renderer's compiled messages from there, so it carries this table:
// one entry per locale the app ships, the same keys in each.
//
// The table is those two dialogs and no more. The pages the OAuth
// callback server answers the browser with (oauth-server.js) are the
// main process's text too, and are still English. lib/main-messages.test.cjs holds the list to the renderer's
// (frontend/project.inlang/settings.json), so adding a locale there
// turns this red until it is added here.

const BASE_LOCALE = "en";

const MESSAGES = {
  en: {
    bootFailed: "ani-gui could not start",
    bootFailedDetail: (reason) =>
      `The app ran into a problem while starting and has to close.\n\nDetails: ${reason}`,
    close: "Close",
    downloadsTitle: "Active downloads",
    downloadsInProgress: (count) =>
      count === 1 ? "1 download in progress." : `${count} downloads in progress.`,
    downloadsQuitDetail: "They will be cancelled if you quit. Continue?",
    cancel: "Cancel",
    quitAnyway: "Quit anyway",
  },
  "pt-BR": {
    bootFailed: "Não foi possível iniciar o ani-gui",
    bootFailedDetail: (reason) =>
      `O aplicativo encontrou um problema ao iniciar e precisa ser fechado.\n\nDetalhes: ${reason}`,
    close: "Fechar",
    downloadsTitle: "Downloads ativos",
    downloadsInProgress: (count) =>
      count === 1 ? "1 download em andamento." : `${count} downloads em andamento.`,
    downloadsQuitDetail: "Eles serão cancelados se você sair. Continuar?",
    cancel: "Cancelar",
    quitAnyway: "Sair mesmo assim",
  },
  "es-419": {
    bootFailed: "No se pudo iniciar ani-gui",
    bootFailedDetail: (reason) =>
      `La aplicación tuvo un problema al iniciar y debe cerrarse.\n\nDetalles: ${reason}`,
    close: "Cerrar",
    downloadsTitle: "Descargas activas",
    downloadsInProgress: (count) =>
      count === 1 ? "1 descarga en curso." : `${count} descargas en curso.`,
    downloadsQuitDetail: "Se cancelarán si sales. ¿Continuar?",
    cancel: "Cancelar",
    quitAnyway: "Salir de todos modos",
  },
  ru: {
    bootFailed: "Не удалось запустить ani-gui",
    bootFailedDetail: (reason) =>
      `При запуске приложения возникла проблема, и оно будет закрыто.\n\nПодробности: ${reason}`,
    close: "Закрыть",
    downloadsTitle: "Активные загрузки",
    // Counted after a colon, so the noun keeps one form for any
    // number; Russian would otherwise need three.
    downloadsInProgress: (count) => `Активных загрузок: ${count}.`,
    downloadsQuitDetail: "При выходе они будут отменены. Продолжить?",
    cancel: "Отмена",
    quitAnyway: "Всё равно выйти",
  },
};

const LOCALES = Object.keys(MESSAGES);

/** The shipped locale `tag` names, compared without regard to case. */
function shippedLocale(tag) {
  if (typeof tag !== "string" || tag === "") return undefined;
  const wanted = tag.toLowerCase();
  return LOCALES.find((locale) => locale.toLowerCase() === wanted);
}

/**
 * The locale the main process speaks in: the one the renderer comes
 * up in, by the renderer's own order.
 *
 *   1. `configured`, the locale saved in config.toml, when it is
 *      exactly a locale the app ships. The renderer applies it at
 *      boot (frontend/src/hooks.client.ts) and compares it as
 *      written, so a value in the wrong case falls through there and
 *      has to here.
 *   2. The system's languages, as the renderer sees them in
 *      `navigator.languages`: `appLocale` (Electron's
 *      `app.getLocale()`) first, then `preferred`
 *      (`app.getPreferredSystemLanguages()`). The first that matches
 *      a shipped locale by its full tag or, failing that, by its base
 *      language decides, compared without regard to case — Paraglide's
 *      `preferredLanguage` strategy. The application locale matters:
 *      it is Chromium's pick among the locales it ships, and it turns
 *      `es-MX` and every other Latin American Spanish into `es-419`,
 *      which nothing in the preferred list would match.
 *   3. The base locale.
 *
 * One thing the renderer consults is out of reach here: its own
 * localStorage, where Paraglide remembers the last locale it was
 * set to. config.toml is written whenever that changes, so the two
 * agree unless the file is edited by hand afterwards.
 */
function resolveLocale({ configured, appLocale, preferred } = {}) {
  if (LOCALES.includes(configured)) return configured;
  for (const tag of [appLocale, ...(preferred || [])]) {
    if (typeof tag !== "string") continue;
    const match = shippedLocale(tag) || shippedLocale(tag.split("-")[0]);
    if (match) return match;
  }
  return BASE_LOCALE;
}

/** The table for `locale`, or the base locale's when it has none. */
function messagesFor(locale) {
  return MESSAGES[shippedLocale(locale) || BASE_LOCALE];
}

module.exports = { LOCALES, messagesFor, resolveLocale };
