"use strict";

const test = require("node:test");
const assert = require("node:assert");

const { isDevProfile } = require("./dev-profile.cjs");

test("ELECTRON_DEV=1 selects the dev profile", () => {
  assert.strictEqual(isDevProfile({ ELECTRON_DEV: "1" }), true);
});

test("non-empty ANI_GUI_DEV selects the dev profile without ELECTRON_DEV", () => {
  // A packaged/release build launched with ANI_GUI_DEV=1 (documented for
  // testing migrations against throwaway data) must put Electron's
  // config/userData on the dev profile too, matching the backend — else
  // locale + OAuth tokens leak across the profile boundary.
  assert.strictEqual(isDevProfile({ ANI_GUI_DEV: "1" }), true);
});

test("neither flag set → installed profile", () => {
  assert.strictEqual(isDevProfile({}), false);
});

test("empty ANI_GUI_DEV is treated as unset", () => {
  assert.strictEqual(isDevProfile({ ANI_GUI_DEV: "" }), false);
});
