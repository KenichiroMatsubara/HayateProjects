import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { parse } from 'yaml';
import { describe, expect, it } from 'vitest';

/**
 * `torimi` CLI の bin を使うワークフローは、CLI をビルドした**後に bin を張り直す**契約。
 *
 * CLI の bin 実体は `Torimi/cli/dist/cli.js`。fresh clone では最初の `pnpm install` の時点で
 * まだビルドされておらず、**pnpm は bin ターゲットが無いと `.bin/torimi` の作成を warning で
 * 黙ってスキップする**。後から CLI をビルドしても、完了済みの install は link フェーズを
 * やり直さない（`pnpm install` は "Already up to date" で抜ける）。結果として
 * `pnpm exec torimi …` が `Command "torimi" not found` で落ちる。
 *
 * ローカルでは一度 dist を作った後の install が bin を張っているので**再現しない** — CI の
 * fresh clone だけで落ちる種類の破れで、しかも Playwright の webServer 経由だと
 * 「Exit code: 1」しか出ずに理由が残らない。実際 shogi-demo の e2e ワークフローは
 * この一点で初回実行から落ちた。torimi-release.yml は同じ罠を踏んで同じ手順で直している。
 *
 * ワークフローを跨る規律なのでどのパッケージにも属さない。ここで**全ワークフローを走査して**
 * 固定する（新しいワークフローが同じ穴を掘り直せないように）。
 */

const repoRoot = join(import.meta.dirname, '..', '..', '..');
const WORKFLOW_DIR = '.github/workflows';

interface Step {
  readonly run?: string;
}

/** ワークフローの `run` 文字列を、ジョブ内の宣言順に平坦化して返す。 */
function runsOf(source: string): string[] {
  const workflow = parse(source) as Record<string, unknown>;
  const jobs = (workflow['jobs'] ?? {}) as Record<string, { steps?: Step[] }>;
  return Object.values(jobs).flatMap((job) =>
    (job.steps ?? []).map((step) => String(step.run ?? '')),
  );
}

/** `torimi` の bin を叩く run か（`torimi build|dev` を直接／スクリプト越しに回す）。 */
function invokesTorimiBin(run: string): boolean {
  return /(^|[\s`"'])torimi\s+(build|dev)\b/.test(run) || run.includes('build:demos');
}

/** bin を張り直す run か（example の node_modules を落として install し直す）。 */
function relinksBins(run: string): boolean {
  return run.includes('Tsubame/examples/*/node_modules') && run.includes('pnpm install');
}

function workflowFiles(): string[] {
  return readdirSync(join(repoRoot, WORKFLOW_DIR)).filter((f) => f.endsWith('.yml'));
}

describe('torimi CLI bin re-link contract', () => {
  it('走査対象のワークフローが実在する（グロブが空振りしていない）', () => {
    expect(workflowFiles().length).toBeGreaterThan(0);
  });

  it('`torimi` の bin を回すワークフローは、CLI ビルドの後に bin を張り直してから使う', () => {
    const offenders: string[] = [];

    for (const file of workflowFiles()) {
      const source = readFileSync(join(repoRoot, WORKFLOW_DIR, file), 'utf8');
      const runs = runsOf(source);

      // webServer 経由など、run ではなく設定ファイル側から起動されることもある。
      // ここで見るのは「ワークフローの run が bin を必要とするか」だけ。
      const firstUse = runs.findIndex(invokesTorimiBin);
      if (firstUse < 0) continue;

      const relink = runs.findIndex(relinksBins);
      if (relink < 0) {
        offenders.push(`${file}: torimi の bin を使うのに bin の張り直しが無い`);
        continue;
      }
      if (relink > firstUse) {
        offenders.push(`${file}: bin の張り直しが最初の torimi 実行より後ろにある`);
      }
    }

    expect(
      offenders,
      'fresh clone の CI で `Command "torimi" not found` になる。CLI をビルドした後に ' +
        '`rm -rf Tsubame/examples/*/node_modules && pnpm install --frozen-lockfile` を挟むこと',
    ).toEqual([]);
  });

  it('e2e が `torimi dev` を webServer で起動するワークフローも同じ契約に入る', () => {
    // shogi-demo の e2e は playwright.config.ts の webServer が `torimi dev web` を起動する。
    // run 文字列には現れないので、上の走査だけでは漏れる — 明示的に押さえる。
    const file = 'shogi-demo-e2e.yml';
    const runs = runsOf(readFileSync(join(repoRoot, WORKFLOW_DIR, file), 'utf8'));

    const config = readFileSync(
      join(repoRoot, 'Tsubame/examples/shogi-demo/playwright.config.ts'),
      'utf8',
    );
    expect(config, 'この spec の前提: webServer が torimi dev web を起動する').toContain(
      'torimi dev web',
    );

    const relink = runs.findIndex(relinksBins);
    const e2e = runs.findIndex((run) => run.includes('test:e2e'));
    expect(relink, `${file}: bin の張り直しが要る`).toBeGreaterThanOrEqual(0);
    expect(e2e, `${file}: e2e を回す step が要る`).toBeGreaterThanOrEqual(0);
    expect(relink, `${file}: 張り直しは e2e より前`).toBeLessThan(e2e);
  });
});
