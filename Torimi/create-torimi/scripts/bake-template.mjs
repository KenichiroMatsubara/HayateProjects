#!/usr/bin/env node
// bake-template.mjs — copy the in-monorepo template canonical source into
// dist/template, replacing __TORIMI_VERSION__ with create-torimi's OWN version
// (#772 / ADR-0008 §6). Runs as part of `build`, so `create-torimi@X` always ships
// a template pinned to the X train — with zero manual version edits. The changeset
// version bump (which sets this package's version) is the only input.
import { readFileSync, rmSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { bakeVersion, copyTreeWithTransform } from '../dist/index.js';

const pkgRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const templateSrc = join(pkgRoot, '..', 'templates', 'solid');
const templateOut = join(pkgRoot, 'dist', 'template');

const version = JSON.parse(readFileSync(join(pkgRoot, 'package.json'), 'utf8')).version;

rmSync(templateOut, { recursive: true, force: true });
copyTreeWithTransform(templateSrc, templateOut, (content) => bakeVersion(content, version));

console.log(`bake-template: template pinned to version ${version} → ${templateOut}`);
