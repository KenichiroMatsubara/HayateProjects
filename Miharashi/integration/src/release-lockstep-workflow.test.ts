import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { parse } from 'yaml';
import { describe, expect, it } from 'vitest';

/**
 * リリース lockstep CI（ADR-0003 / #741）のワークフロー契約テスト。
 *
 * Demo Endpoint のデプロイは Play リリースと lockstep — 起点は `miharashi-android-v*` タグと
 * 手動実行（workflow_dispatch）**のみ**で、main への push ではデプロイしない（main 上の
 * Protocol Version 更新が Play 配布済みホストを一斉に明示エラーへ落とすのを「構造」で防ぐ）。
 * ワークフローは example / demo-endpoint / CI を跨る合成なので、どのパッケージにも属さない
 * この統合テストが YAML を解析して契約を固定する（Rust 側の wiring テストと同じ流儀）。
 */

/** リリース lockstep ワークフローの置き場所（リポジトリルート相対）。 */
const WORKFLOW_PATH = '.github/workflows/miharashi-release.yml';
/** リリースタグの名前パターン（ADR-0003）。AAB ビルドも将来同じタグに乗る。 */
const RELEASE_TAG_PATTERN = 'miharashi-android-v*';

const repoRoot = join(import.meta.dirname, '..', '..', '..');

function workflowSource(): string {
  return readFileSync(join(repoRoot, WORKFLOW_PATH), 'utf8');
}

// YAML 1.1 の罠：素の `on:` キーは boolean true に解釈される。GitHub Actions 自身は `on` を
// 文字列キーとして扱うので、parse 結果のどちらのキー名でも拾えるようにする。
function triggersOf(workflow: Record<string, unknown>): Record<string, unknown> {
  const on = workflow['on'] ?? workflow['true'];
  expect(on, 'the workflow must declare its triggers').toBeTruthy();
  return on as Record<string, unknown>;
}

describe('release lockstep workflow (ADR-0003)', () => {
  it('is triggered only by the release tag pattern and workflow_dispatch — never by push to main', () => {
    const workflow = parse(workflowSource()) as Record<string, unknown>;
    const triggers = triggersOf(workflow);

    // トリガはこの 2 つだけ。push/main や schedule が紛れたら lockstep が破れる。
    expect(Object.keys(triggers).sort()).toEqual(['push', 'workflow_dispatch']);

    // push トリガはタグに限定する（branches キーが在ること自体が main 追従の兆候）。
    const push = triggers['push'] as Record<string, unknown>;
    expect(Object.keys(push)).toEqual(['tags']);
    expect(push['tags']).toEqual([RELEASE_TAG_PATTERN]);
  });

  it('builds the demo bundles from the tagged commit and deploys them with wrangler', () => {
    const workflow = parse(workflowSource()) as Record<string, unknown>;
    const jobs = workflow['jobs'] as Record<string, { steps: Array<Record<string, unknown>> }>;
    const deploy = jobs['deploy-demo-endpoint'];
    if (deploy == null) throw new Error('the demo deploy job must exist');

    const runs = deploy.steps.map((step) => String(step['run'] ?? ''));

    // デモバンドルは demo-endpoint の build:demos が demos.json（solid / react の正本）を
    // 読んで各パッケージの build:android を回す — ワークフローにデモ一覧を複製しない。
    expect(runs.some((run) => run.includes('build:demos'))).toBe(true);

    // デプロイは wrangler（demo-endpoint の deploy スクリプト）。
    const deployStep = deploy.steps.find((step) => String(step['run'] ?? '').includes('run deploy'));
    expect(deployStep, 'a wrangler deploy step must exist').toBeTruthy();

    // wrangler が要る Cloudflare 資格情報は GitHub Secrets から渡す（README に登録手順）。
    const deployEnv = deployStep?.['env'] as Record<string, string>;
    expect(deployEnv['CLOUDFLARE_API_TOKEN']).toBe('${{ secrets.CLOUDFLARE_API_TOKEN }}');
    expect(deployEnv['CLOUDFLARE_ACCOUNT_ID']).toBe('${{ secrets.CLOUDFLARE_ACCOUNT_ID }}');

    // build:demos は example の vite build を回す — workspace 依存（@hayate/host / Tsubame
    // packages）の dist が要るので、先に組み立てる（deploy-pages.yml と同じ順序）。
    const bundleBuildIndex = runs.findIndex((run) => run.includes('build:demos'));
    const hostBuildIndex = runs.findIndex((run) => run.includes('@hayate/host'));
    const tsubameBuildIndex = runs.findIndex((run) => run.includes('./Tsubame/packages/*'));
    expect(hostBuildIndex).toBeGreaterThanOrEqual(0);
    expect(tsubameBuildIndex).toBeGreaterThanOrEqual(0);
    expect(hostBuildIndex).toBeLessThan(bundleBuildIndex);
    expect(tsubameBuildIndex).toBeLessThan(bundleBuildIndex);
  });

  it('documents the required Cloudflare secrets and how to register them in the demo-endpoint README', () => {
    // 登録作業そのものは完全人力スライスの領分 — ワークフローが期待する Secrets 名と
    // 登録手順が README に明記されていることだけを固定する（#741 受け入れ基準）。
    const readme = readFileSync(join(repoRoot, 'Miharashi/demo-endpoint/README.md'), 'utf8');
    for (const secret of ['CLOUDFLARE_API_TOKEN', 'CLOUDFLARE_ACCOUNT_ID']) {
      expect(readme, `README must document the ${secret} secret`).toContain(secret);
    }
    // GitHub 側の登録先（Actions secrets）と、トークンを Workers 編集権限に絞ることが読めること。
    expect(readme).toMatch(/Secrets and variables/i);
    expect(readme).toMatch(/Workers/);
  });
});
