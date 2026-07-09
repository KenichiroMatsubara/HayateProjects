import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { afterEach, describe, expect, it } from 'vitest';

import {
  PROJECT_NAME_TOKEN,
  TEMPLATE_VERSION_TOKEN,
  applyProjectName,
  bakeVersion,
  isTextFile,
  scaffold,
  scaffoldFileName,
  validateProjectName,
} from './scaffold.js';

describe('token substitution', () => {
  it('bakeVersion replaces every version token (publish-time pin)', () => {
    const pkg = `{ "@torimi/bundle": "${TEMPLATE_VERSION_TOKEN}", "torimi": "${TEMPLATE_VERSION_TOKEN}" }`;
    expect(bakeVersion(pkg, '0.1.0')).toBe('{ "@torimi/bundle": "0.1.0", "torimi": "0.1.0" }');
    expect(bakeVersion(pkg, '0.1.0')).not.toContain(TEMPLATE_VERSION_TOKEN);
  });

  it('applyProjectName replaces every project-name token', () => {
    expect(applyProjectName(`name: ${PROJECT_NAME_TOKEN}`, 'my-app')).toBe('name: my-app');
  });
});

describe('file classification', () => {
  it('treats source/config/gitignore as text and other files as binary', () => {
    expect(isTextFile('App.tsx')).toBe(true);
    expect(isTextFile('package.json')).toBe(true);
    expect(isTextFile('gitignore')).toBe(true);
    expect(isTextFile('logo.png')).toBe(false);
  });

  it('restores the leading dot on gitignore at scaffold time', () => {
    expect(scaffoldFileName('gitignore')).toBe('.gitignore');
    expect(scaffoldFileName('package.json')).toBe('package.json');
  });
});

describe('validateProjectName', () => {
  it('accepts ordinary names and rejects paths / dotfiles / empties', () => {
    expect(() => validateProjectName('my-app')).not.toThrow();
    expect(() => validateProjectName('')).toThrow();
    expect(() => validateProjectName('.hidden')).toThrow();
    expect(() => validateProjectName('a/b')).toThrow();
  });
});

describe('scaffold (copy + name substitution, no network)', () => {
  const dirs: string[] = [];
  afterEach(() => dirs.splice(0).forEach((d) => rmSync(d, { recursive: true, force: true })));

  it('copies a version-baked template, substitutes the name, and restores dotfiles', () => {
    const root = mkdtempSync(join(tmpdir(), 'create-torimi-test-'));
    dirs.push(root);
    const templateDir = join(root, 'template');
    mkdirSync(join(templateDir, 'src'), { recursive: true });
    // A version-baked template (version already substituted; project name still a token).
    writeFileSync(join(templateDir, 'package.json'), `{ "name": "${PROJECT_NAME_TOKEN}", "dependencies": { "torimi": "0.1.0" } }`);
    writeFileSync(join(templateDir, 'src', 'App.tsx'), `// ${PROJECT_NAME_TOKEN}\nexport const App = () => null;\n`);
    writeFileSync(join(templateDir, 'gitignore'), 'node_modules\n');

    const target = join(root, 'my-app');
    scaffold(templateDir, target, 'my-app');

    const pkg = readFileSync(join(target, 'package.json'), 'utf8');
    expect(pkg).toContain('"name": "my-app"');
    expect(pkg).not.toContain(PROJECT_NAME_TOKEN);
    expect(readFileSync(join(target, 'src', 'App.tsx'), 'utf8')).toContain('// my-app');
    // gitignore restored to .gitignore; no leftover `gitignore`.
    const top = readdirSync(target);
    expect(top).toContain('.gitignore');
    expect(top).not.toContain('gitignore');
  });
});
