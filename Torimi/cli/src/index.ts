// Library surface for the Torimi CLI internals — exported so the pure helpers
// (config normalization, target/port resolution, lowering) are importable and
// unit-testable. The user-facing entry is the `torimi` bin (cli.ts).
export {
  DEFAULT_TARGET,
  DEFAULT_WATCH_DIR,
  NATIVE_DEV_PORT,
  REBUILD_DEBOUNCE_MS,
  TARGETS,
  WEB_DEV_PORT,
  loweredBundlePath,
  portForTarget,
  resolveTarget,
  type Target,
} from './constants.js';
export { findConfigPath, loadTorimiConfig, normalizeConfig, type TorimiConfig } from './config.js';
export { countClassKeywords, lowerFileTo, lowerForHermes, type LowerResult } from './lower.js';
export { buildForTarget, runShell } from './build.js';
export { dev, type DevOptions } from './dev.js';
