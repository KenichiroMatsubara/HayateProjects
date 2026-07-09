// src/constants.ts
import { extname } from "path";
var TARGETS = ["native", "web"];
var DEFAULT_TARGET = "native";
var NATIVE_DEV_PORT = 5179;
var WEB_DEV_PORT = 5181;
var DEFAULT_WATCH_DIR = "src";
var REBUILD_DEBOUNCE_MS = 120;
function portForTarget(target) {
  return target === "native" ? NATIVE_DEV_PORT : WEB_DEV_PORT;
}
function resolveTarget(arg) {
  if (arg === void 0) return DEFAULT_TARGET;
  if (TARGETS.includes(arg)) return arg;
  throw new Error(`torimi: unknown target "${arg}" (expected one of: ${TARGETS.join(", ")})`);
}
function loweredBundlePath(bundle) {
  const ext = extname(bundle);
  return ext ? `${bundle.slice(0, -ext.length)}.hermes${ext}` : `${bundle}.hermes`;
}

// src/lower.ts
import { readFile, writeFile } from "fs/promises";
import { transformAsync } from "@babel/core";
import presetEnv from "@babel/preset-env";
async function lowerForHermes(code) {
  const out = await transformAsync(code, {
    babelrc: false,
    configFile: false,
    compact: false,
    presets: [[presetEnv, { targets: { ie: "11" }, modules: false }]]
  });
  if (!out?.code) throw new Error("torimi lower: babel produced no output");
  return out.code;
}
function countClassKeywords(code) {
  return (code.match(/(?<![.\w])class\b/g) || []).length;
}
async function lowerFileTo(src, dest) {
  const lowered = await lowerForHermes(await readFile(src, "utf8"));
  await writeFile(dest, lowered);
  return { classKeywordsLeft: countClassKeywords(lowered), size: lowered.length };
}

// src/build.ts
import { spawn } from "child_process";
import { join } from "path";
function runShell(command, cwd) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, { cwd, stdio: "inherit", shell: true });
    child.on("exit", (code) => {
      if (code === 0) resolve();
      else reject(new Error(`torimi: build command failed (exit ${code}): ${command}`));
    });
    child.on("error", reject);
  });
}
async function buildForTarget(config, target, cwd) {
  await runShell(config.build, cwd);
  const bundleAbs = join(cwd, config.bundle);
  if (target === "web") return bundleAbs;
  const loweredAbs = join(cwd, loweredBundlePath(config.bundle));
  const { classKeywordsLeft, size } = await lowerFileTo(bundleAbs, loweredAbs);
  console.log(`torimi: lowered for Hermes (class keywords left: ${classKeywordsLeft}, size ${size})`);
  return loweredAbs;
}

// src/config.ts
import { access } from "fs/promises";
import { join as join2 } from "path";
import { pathToFileURL } from "url";
var CONFIG_BASENAMES = ["torimi.config.mjs", "torimi.config.js"];
async function findConfigPath(cwd) {
  for (const name of CONFIG_BASENAMES) {
    const candidate = join2(cwd, name);
    try {
      await access(candidate);
      return candidate;
    } catch {
    }
  }
  throw new Error(`torimi: no ${CONFIG_BASENAMES.join(" or ")} found in ${cwd}`);
}
function normalizeConfig(raw) {
  if (!raw || typeof raw !== "object") throw new Error("torimi.config: default export must be an object");
  const { build, bundle, watch: watch2 } = raw;
  if (typeof build !== "string" || build.trim() === "") {
    throw new Error("torimi.config: `build` must be a non-empty string");
  }
  if (typeof bundle !== "string" || bundle.trim() === "") {
    throw new Error("torimi.config: `bundle` must be a non-empty string");
  }
  if (watch2 !== void 0 && typeof watch2 !== "string") {
    throw new Error("torimi.config: `watch` must be a string");
  }
  return { build, bundle, watch: watch2 ?? DEFAULT_WATCH_DIR };
}
async function loadTorimiConfig(cwd) {
  const path = await findConfigPath(cwd);
  const mod = await import(pathToFileURL(path).href);
  return normalizeConfig(mod.default ?? mod);
}

// src/dev.ts
import { watch } from "fs";
import { join as join4 } from "path";
import { ALL_INTERFACES_HOSTNAME, createBundleDevServer, printStartupBanner } from "@torimi/dev-server";

// src/port.ts
import { execFileSync } from "child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "fs";
import { connect } from "net";
import { dirname, join as join3 } from "path";
function pidFilePath(cwd, port) {
  return join3(cwd, "node_modules", ".cache", "torimi", `dev-${port}.pid`);
}
function processCommand(pid) {
  try {
    return execFileSync("ps", ["-o", "command=", "-p", String(pid)], { encoding: "utf8" });
  } catch {
    return null;
  }
}
function reclaimStaleDevServer(cwd, port) {
  const file = pidFilePath(cwd, port);
  if (!existsSync(file)) return;
  let pid;
  try {
    pid = Number(readFileSync(file, "utf8").trim());
  } catch {
    return;
  }
  if (!Number.isInteger(pid) || pid <= 0 || pid === process.pid) return;
  const cmd = processCommand(pid);
  if (!cmd || !/torimi/.test(cmd)) return;
  try {
    process.kill(pid, "SIGTERM");
    console.log(`torimi dev: reclaimed a stale dev server (PID ${pid}) holding port ${port}.`);
  } catch {
  }
}
function writePidFile(cwd, port) {
  const file = pidFilePath(cwd, port);
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, String(process.pid));
  return file;
}
function removePidFile(file) {
  try {
    rmSync(file);
  } catch {
  }
}
function isPortInUse(port) {
  return new Promise((resolve) => {
    const socket = connect({ port, host: "127.0.0.1" });
    socket.once("connect", () => {
      socket.destroy();
      resolve(true);
    });
    socket.once("error", () => resolve(false));
  });
}
async function waitForPortFree(port, attempts = 10, delayMs = 200) {
  for (let i = 0; i < attempts; i += 1) {
    if (!await isPortInUse(port)) return;
    await new Promise((r) => setTimeout(r, delayMs));
  }
}

// src/dev.ts
async function dev(config, target, cwd, options = {}) {
  const port = options.port ?? portForTarget(target);
  let building = false;
  let queued = false;
  let debounce;
  let servedPath = "";
  async function rebuild() {
    if (building) {
      queued = true;
      return;
    }
    building = true;
    do {
      queued = false;
      try {
        servedPath = await buildForTarget(config, target, cwd);
      } catch (err) {
        console.error(`torimi dev: build failed (${String(err)}) \u2014 save again to retry`);
      }
    } while (queued);
    building = false;
  }
  function scheduleRebuild() {
    if (debounce != null) clearTimeout(debounce);
    debounce = setTimeout(() => {
      debounce = void 0;
      void rebuild();
    }, REBUILD_DEBOUNCE_MS);
  }
  console.log(`torimi dev (${target}): initial build\u2026`);
  await rebuild();
  if (!servedPath) throw new Error("torimi dev: initial build failed \u2014 cannot start dev server");
  const watchDir = join4(cwd, config.watch);
  const watcher = watch(watchDir, { recursive: true }, () => scheduleRebuild());
  reclaimStaleDevServer(cwd, port);
  await waitForPortFree(port);
  const server = createBundleDevServer({ bundlePath: servedPath, port, hostname: ALL_INTERFACES_HOSTNAME });
  try {
    await server.listen();
  } catch (err) {
    const code = err?.code ?? err;
    console.error(`torimi dev: could not listen on port ${port} (${code}).`);
    console.error("  Another process may be using this port (override with TORIMI_DEV_PORT).");
    watcher.close();
    process.exit(1);
  }
  const pidFile = writePidFile(cwd, port);
  printStartupBanner({ port, loopbackUrl: `http://localhost:${port}` });
  const shutdown = () => {
    if (debounce != null) clearTimeout(debounce);
    watcher.close();
    removePidFile(pidFile);
    server.close().finally(() => process.exit(0));
  };
  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);
}

export {
  TARGETS,
  DEFAULT_TARGET,
  NATIVE_DEV_PORT,
  WEB_DEV_PORT,
  DEFAULT_WATCH_DIR,
  REBUILD_DEBOUNCE_MS,
  portForTarget,
  resolveTarget,
  loweredBundlePath,
  lowerForHermes,
  countClassKeywords,
  lowerFileTo,
  runShell,
  buildForTarget,
  findConfigPath,
  normalizeConfig,
  loadTorimiConfig,
  dev
};
//# sourceMappingURL=chunk-VQM7KIQK.js.map