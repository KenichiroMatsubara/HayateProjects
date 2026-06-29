// `pnpm miharashi:android` — Miharashi を端末で回すワンコマンド。
//
// やること（コンカレント）:
//   1. 最新の Miharashi Android ホスト（事前ビルド済みネイティブシェル）を接続中の
//      端末 / エミュレータへインストールする（`pnpm --filter hayate android:install`）。
//   2. Miharashi の dev-server（vite build watch + HTTP 配信 + reload WS）を立て続ける
//      （`@tsubame/example-todo` の miharashi:android:serve）。ホストはここから App Bundle を
//      実行時 fetch し、ソース変更で WS reload が飛ぶ（Miharashi ADR-0001 / CONTEXT.md）。
//
// ホストは「再ビルドせずバンドルだけ差し替える」dev-client（Expo Go 相当）なので、host の
// install は基本 1 回・dev-server は回しっぱなしにする。install が終わっても dev-server は
// 落とさない（端末で URL を入れて起動 → fetch → hot reload まで使い続けるため）。
//
// 注意:
//   - android:install には Android SDK / NDK と接続中の端末（adb で見える実機 or エミュレータ）が要る。
//   - dev-server は既定で 5179（ネイティブ既定ポート）。MIHARASHI_DEV_PORT で変更可。
//   - 配信アプリは既定で solid の example-todo。MIHARASHI_EXAMPLE で pnpm filter を上書き可
//     （例: MIHARASHI_EXAMPLE=@tsubame/example-react-todo pnpm miharashi:android）。
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('..', import.meta.url));
const example = process.env.MIHARASHI_EXAMPLE ?? '@tsubame/example-todo';

const RESET = '\x1b[0m';

/**
 * 子プロセスを起動し、行頭に色付きラベルを付けて親の stdout/stderr に転送する。
 * label でログの出所（serve / install）を見分けられるようにする。
 */
function run(label, color, command, args) {
  const child = spawn(command, args, { cwd: root });
  const prefix = `${color}[${label}]${RESET} `;
  const pipe = (stream, out) => {
    let buffer = '';
    stream.setEncoding('utf8');
    stream.on('data', (chunk) => {
      buffer += chunk;
      const lines = buffer.split('\n');
      buffer = lines.pop() ?? '';
      for (const line of lines) out.write(prefix + line + '\n');
    });
    stream.on('end', () => {
      if (buffer.length > 0) out.write(prefix + buffer + '\n');
    });
  };
  pipe(child.stdout, process.stdout);
  pipe(child.stderr, process.stderr);
  return child;
}

// dev-server（回しっぱなし）と host install（基本 1 回）をコンカレントに起動する。
const serve = run('serve', '\x1b[36m', 'pnpm', [
  '--filter',
  example,
  'run',
  'miharashi:android:serve',
]);
const install = run('install', '\x1b[35m', 'pnpm', [
  '--filter',
  'hayate',
  'run',
  'android:install',
]);

// install の結果を知らせる。失敗（SDK/端末なし等）でも dev-server は止めない
// ——SDK を直すか端末を繋いでから別端末で install し直せるよう、配信は生かす。
install.on('exit', (code) => {
  if (code === 0) {
    console.log('\x1b[32m✓ Miharashi ホストを端末へインストールしました。\x1b[0m');
    console.log('  端末でアプリを開き、dev-server URL（上の host:port）を入力して起動してください。');
  } else {
    console.error(`\x1b[31m✗ android:install が exit ${code} で失敗しました。\x1b[0m`);
    console.error('  Android SDK/NDK と接続中の端末（adb devices）を確認してください。dev-server は起動したままです。');
  }
});

// どちらかの致命終了 / Ctrl-C で両方を確実に畳む。
let shuttingDown = false;
function shutdown(exitCode) {
  if (shuttingDown) return;
  shuttingDown = true;
  for (const child of [serve, install]) {
    if (child.exitCode == null && child.signalCode == null) child.kill();
  }
  process.exit(exitCode);
}

process.on('SIGINT', () => shutdown(0));
process.on('SIGTERM', () => shutdown(0));

// dev-server が落ちたら命脈が尽きるので全体を終了する（install 単独の終了は許容する）。
serve.on('exit', (code) => {
  console.error(`\x1b[31mMiharashi dev-server が終了しました（exit ${code ?? 'signal'}）。\x1b[0m`);
  shutdown(code ?? 1);
});
