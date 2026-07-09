import { cpSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

// Placeholders baked/substituted into the template. The version token is replaced
// at PUBLISH time (bake-template.mjs) so `create-torimi@X` always emits the X train
// (no GitHub fetch at generation). The project-name token is replaced at generation.
export const TEMPLATE_VERSION_TOKEN = '__TORIMI_VERSION__';
export const PROJECT_NAME_TOKEN = '__PROJECT_NAME__';

// Extensions whose contents get token substitution. Binary/asset files are copied
// verbatim. Extensionless names handled explicitly (gitignore).
const TEXT_EXTENSIONS = new Set(['.ts', '.tsx', '.mjs', '.js', '.json', '.md', '.html', '.css']);

export function isTextFile(name: string): boolean {
  if (name === 'gitignore' || name === '.gitignore') return true;
  const dot = name.lastIndexOf('.');
  return dot >= 0 && TEXT_EXTENSIONS.has(name.slice(dot));
}

// A template ships `gitignore` (no dot) so npm doesn't strip it from the tarball;
// the scaffold restores the leading dot on the way out.
export function scaffoldFileName(basename: string): string {
  return basename === 'gitignore' ? '.gitignore' : basename;
}

export function bakeVersion(content: string, version: string): string {
  return content.split(TEMPLATE_VERSION_TOKEN).join(version);
}

export function applyProjectName(content: string, projectName: string): string {
  return content.split(PROJECT_NAME_TOKEN).join(projectName);
}

// npm-ish project/dir name: non-empty, no path separators or spaces, not a dotfile.
export function validateProjectName(name: string): void {
  if (!name || !/^[a-z0-9._-]+$/i.test(name) || name.startsWith('.')) {
    throw new Error(`create-torimi: invalid project name "${name}" (use letters, digits, -, _)`);
  }
}

// Recursively copy `srcDir` → `destDir`, running `transform(text, relPath)` on text
// files and `renameBasename` on each file name. Shared by bake (build) and scaffold
// (generation).
export function copyTreeWithTransform(
  srcDir: string,
  destDir: string,
  transform: (content: string, name: string) => string,
  renameBasename: (basename: string) => string = (b) => b,
): void {
  mkdirSync(destDir, { recursive: true });
  for (const entry of readdirSync(srcDir)) {
    const srcPath = join(srcDir, entry);
    if (statSync(srcPath).isDirectory()) {
      copyTreeWithTransform(srcPath, join(destDir, entry), transform, renameBasename);
      continue;
    }
    const destPath = join(destDir, renameBasename(entry));
    if (isTextFile(entry)) {
      writeFileSync(destPath, transform(readFileSync(srcPath, 'utf8'), entry));
    } else {
      cpSync(srcPath, destPath);
    }
  }
}

// Generation-time: copy the (already version-baked) bundled template into the user's
// new project dir, substituting the project name and restoring dotfiles. No network.
export function scaffold(templateDir: string, targetDir: string, projectName: string): void {
  validateProjectName(projectName);
  copyTreeWithTransform(templateDir, targetDir, (content) => applyProjectName(content, projectName), scaffoldFileName);
}
