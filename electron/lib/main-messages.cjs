"use strict";

// The main process's own user-visible text. Placeholder: English
// only, and no way to pick a locale.

const LOCALES = ["en"];

const MESSAGES = {
  en: {
    bootFailed: "ani-gui could not start",
    bootFailedDetail: (reason) => String(reason),
    close: "Close",
  },
};

function resolveLocale() {
  return "en";
}

function messagesFor() {
  return MESSAGES.en;
}

module.exports = { LOCALES, messagesFor, resolveLocale };
