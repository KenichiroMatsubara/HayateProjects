#!/usr/bin/env node
import {
  buildForTarget,
  dev,
  loadTorimiConfig,
  lowerFileTo,
  resolveTarget
} from "./chunk-VQM7KIQK.js";

// src/cli.ts
import { resolve } from "path";
var USAGE = `torimi \u2014 Torimi orchestrator CLI (ADR-0008)

Usage:
  torimi dev [target]     build once, watch sources, serve + live-reload (default target: native)
  torimi build [target]   one-shot build (+ Hermes lowering for native)
  torimi lower <file>     Hermes-lower a built bundle in place (escape hatch)

Targets: native (default) | web
Env: TORIMI_DEV_PORT overrides the dev server port.`;
async function run(argv) {
  const [command, arg] = argv;
  const cwd = process.cwd();
  const portEnv = process.env.TORIMI_DEV_PORT ? Number(process.env.TORIMI_DEV_PORT) : void 0;
  switch (command) {
    case "dev": {
      const target = resolveTarget(arg);
      await dev(await loadTorimiConfig(cwd), target, cwd, { port: portEnv });
      return;
    }
    case "build": {
      const target = resolveTarget(arg);
      const out = await buildForTarget(await loadTorimiConfig(cwd), target, cwd);
      console.log(`torimi: built ${target} \u2192 ${out}`);
      return;
    }
    case "lower": {
      if (!arg) throw new Error("torimi lower: usage: torimi lower <file>");
      const file = resolve(cwd, arg);
      const { classKeywordsLeft, size } = await lowerFileTo(file, file);
      console.log(`torimi lower: ${file} (class keywords left: ${classKeywordsLeft}, size ${size})`);
      return;
    }
    case void 0:
    case "-h":
    case "--help":
      console.log(USAGE);
      return;
    default:
      console.error(`torimi: unknown command "${command}"

${USAGE}`);
      process.exitCode = 1;
  }
}
run(process.argv.slice(2)).catch((err) => {
  console.error(String(err?.stack ?? err));
  process.exitCode = 1;
});
export {
  run
};
//# sourceMappingURL=cli.js.map