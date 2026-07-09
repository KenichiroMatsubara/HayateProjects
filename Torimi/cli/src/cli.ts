#!/usr/bin/env node
import { resolve } from 'node:path';

import { buildForTarget } from './build.js';
import { loadTorimiConfig } from './config.js';
import { resolveTarget } from './constants.js';
import { dev } from './dev.js';
import { lowerFileTo } from './lower.js';

const USAGE = `torimi — Torimi orchestrator CLI (ADR-0008)

Usage:
  torimi dev [target]     build once, watch sources, serve + live-reload (default target: native)
  torimi build [target]   one-shot build (+ Hermes lowering for native)
  torimi lower <file>     Hermes-lower a built bundle in place (escape hatch)

Targets: native (default) | web
Env: TORIMI_DEV_PORT overrides the dev server port.`;

export async function run(argv: string[]): Promise<void> {
  const [command, arg] = argv;
  const cwd = process.cwd();
  const portEnv = process.env.TORIMI_DEV_PORT ? Number(process.env.TORIMI_DEV_PORT) : undefined;

  switch (command) {
    case 'dev': {
      const target = resolveTarget(arg);
      await dev(await loadTorimiConfig(cwd), target, cwd, { port: portEnv });
      return; // dev runs until SIGINT/SIGTERM
    }
    case 'build': {
      const target = resolveTarget(arg);
      const out = await buildForTarget(await loadTorimiConfig(cwd), target, cwd);
      console.log(`torimi: built ${target} → ${out}`);
      return;
    }
    case 'lower': {
      if (!arg) throw new Error('torimi lower: usage: torimi lower <file>');
      const file = resolve(cwd, arg);
      const { classKeywordsLeft, size } = await lowerFileTo(file, file);
      console.log(`torimi lower: ${file} (class keywords left: ${classKeywordsLeft}, size ${size})`);
      return;
    }
    case undefined:
    case '-h':
    case '--help':
      console.log(USAGE);
      return;
    default:
      console.error(`torimi: unknown command "${command}"\n\n${USAGE}`);
      process.exitCode = 1;
  }
}

run(process.argv.slice(2)).catch((err: unknown) => {
  console.error(String((err as { stack?: string })?.stack ?? err));
  process.exitCode = 1;
});
