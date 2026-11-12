// The end-to-end suite's package (`pnpm run package:e2e`): see
// lib/e2e-package.cjs. Never used by a release script.
module.exports = require("./lib/e2e-package.cjs").e2eConfig(require("./package.json").build);
