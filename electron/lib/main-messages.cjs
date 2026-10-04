"use strict";

// The main process's own user-visible text.
//
// Nearly everything the user reads is the renderer's and goes through
// Paraglide. The main process speaks only when there is no renderer to
// speak for it — a boot that failed before a page existed. It cannot
// reach the renderer's compiled messages from there, so it carries
// this table: one entry per locale the app ships, the same keys in
// each. lib/main-messages.test.cjs holds the list to the renderer's
// (frontend/project.inlang/settings.json), so adding a locale there
// turns this red until it is added here.

const BASE_LOCALE = "en";

const MESSAGES = {
  en: {
    bootFailed: "ani-gui could not start",
    bootFailedDetail: (reason) =>
      `The app ran into a problem while starting and has to close.\n\nDetails: ${reason}`,
    close: "Close",
  },
  "pt-BR": {
    bootFailed: "Não foi possível iniciar o ani-gui",
    bootFailedDetail: (reason) =>
      `O aplicativo encontrou um problema ao iniciar e precisa ser fechado.\n\nDetalhes: ${reason}`,
    close: "Fechar",
  },
  "es-419": {
    bootFailed: "No se pudo iniciar ani-gui",
    bootFailedDetail: (reason) =>
      `La aplicación tuvo un problema al iniciar y debe cerrarse.\n\nDetalles: ${reason}`,
    close: "Cerrar",
  },
  ru: {
    bootFailed: "Не удалось запустить ani-gui",
    bootFailedDetail: (reason) =>
      `При запуске приложения возникла проблема, и оно будет закрыто.\n\nПодробности: ${reason}`,
    close: "Закрыть",
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
 * The locale the main process speaks in — the one the renderer would
 * have come up in, by the renderer's own order (Paraglide's
 * `localStorage`, `preferredLanguage`, `baseLocale` strategy, with the
 * preload seeding localStorage from config.toml):
 *
 *   1. `configured`, the locale saved in config.toml, when the app
 *      ships it;
 *   2. the first of `preferred`, the system's languages in order, that
 *      matches a shipped locale by its full tag or, failing that, by
 *      its base language — so `ru-RU` is `ru`, while `es-MX` matches
 *      nothing, `es` alone not being a locale the app ships;
 *   3. the base locale.
 */
function resolveLocale({ configured, preferred } = {}) {
  const saved = shippedLocale(configured);
  if (saved) return saved;
  for (const tag of preferred || []) {
    const match =
      shippedLocale(tag) || shippedLocale(String(tag).split("-")[0]);
    if (match) return match;
  }
  return BASE_LOCALE;
}

/** The table for `locale`, or the base locale's when it has none. */
function messagesFor(locale) {
  return MESSAGES[shippedLocale(locale) || BASE_LOCALE];
}

module.exports = { LOCALES, messagesFor, resolveLocale };
