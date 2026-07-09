#!/usr/bin/env node
import { existsSync } from 'node:fs';
import { basename, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { scaffold, validateProjectName } from './scaffold.js';

// The bundled, version-baked template sits next to this file (dist/template),
// packed at publish time — generation never reaches the network.
const templateDir = fileURLToPath(new URL('./template/', import.meta.url));

const USAGE = `create-torimi — scaffold a new Torimi app

Usage:
  npm create torimi <project-name>
  pnpm create torimi <project-name>

Creates ./<project-name> from the bundled template (Solid). Then:
  cd <project-name> && npm install && npm run dev`;

export function run(argv: string[]): void {
  const projectName = argv[0];
  if (!projectName || projectName === '-h' || projectName === '--help') {
    console.log(USAGE);
    if (!projectName) process.exitCode = 1;
    return;
  }
  validateProjectName(projectName);

  const targetDir = resolve(process.cwd(), projectName);
  if (existsSync(targetDir)) {
    throw new Error(`create-torimi: directory "${basename(targetDir)}" already exists`);
  }

  scaffold(templateDir, targetDir, projectName);

  console.log(`\nScaffolded ${projectName} → ${targetDir}\n`);
  console.log('Next:');
  console.log(`  cd ${projectName}`);
  console.log('  npm install');
  console.log('  npm run dev\n');
}

try {
  run(process.argv.slice(2));
} catch (err) {
  console.error(String((err as { message?: string })?.message ?? err));
  process.exitCode = 1;
}
