#!/usr/bin/env node
// hayate-desktop（winit + vello/wgpu Platform Front・ADR-0118）をビルドして起動する、
// 実機検証用のワンコマンド。デスクトップは GPU surface を実機で見て確かめる必要があるため
// （色・風景・リサイズ追従など）、ビルドと起動を 1 コマンドに畳む。
//
// Windows では前回の窓が起動したままだと出力 .exe がロックされ、再ビルドのリンクが失敗する。
// それを避けるため、ビルド前に残存プロセスを終了する（無ければ無視）。
//
// 使い方:
//   npm run hayate:desktop:run              # ビルドして窓を起動（既定 RUST_LOG=info）
//   npm run hayate:desktop:run -- --release # release ビルドで起動
//   npm run hayate:desktop:build            # ビルドのみ（起動しない・CI/動作確認向け）
import { spawnSync } from "node:child_process";

const argv = process.argv.slice(2);
const buildOnly = argv.includes("--build-only");
const cargoArgs = argv.filter((a) => a !== "--build-only");

if (process.platform === "win32") {
  // 残存する窓を閉じて .exe のロックを外す（起動していなければ何もしない）。
  spawnSync("taskkill", ["/F", "/IM", "hayate-desktop.exe"], { stdio: "ignore" });
}

const verb = buildOnly ? "build" : "run";
const args = [verb, "-p", "hayate-platform-desktop", "--bin", "hayate-desktop", ...cargoArgs];

const res = spawnSync("cargo", args, {
  stdio: "inherit",
  // RUST_LOG 未指定なら info（起動時の surface / wgpu ログを拾えるように）。明示指定は尊重。
  env: { ...process.env, RUST_LOG: process.env.RUST_LOG ?? "info" },
  // Windows では cargo.exe を PATH 経由で解決させる。
  shell: process.platform === "win32",
});

process.exit(res.status ?? 1);
