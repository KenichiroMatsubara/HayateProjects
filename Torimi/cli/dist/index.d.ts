declare const TARGETS: readonly ["native", "web"];
type Target = (typeof TARGETS)[number];
declare const DEFAULT_TARGET: Target;
declare const NATIVE_DEV_PORT = 5179;
declare const WEB_DEV_PORT = 5181;
declare const DEFAULT_WATCH_DIR = "src";
declare const REBUILD_DEBOUNCE_MS = 120;
declare function portForTarget(target: Target): number;
declare function resolveTarget(arg: string | undefined): Target;
declare function loweredBundlePath(bundle: string): string;

interface TorimiConfig {
    /** 一発ビルドコマンド（例 `vite build --config vite.config.torimi.ts`）。CLI は不透明に実行する。 */
    readonly build: string;
    /** build が書き出す単一 App Bundle の、cwd 相対パス。 */
    readonly bundle: string;
    /** `torimi dev` が監視するソースディレクトリ（cwd 相対、既定 `src`）。 */
    readonly watch: string;
}
declare function findConfigPath(cwd: string): Promise<string>;
declare function normalizeConfig(raw: unknown): TorimiConfig;
declare function loadTorimiConfig(cwd: string): Promise<TorimiConfig>;

declare function lowerForHermes(code: string): Promise<string>;
declare function countClassKeywords(code: string): number;
interface LowerResult {
    readonly classKeywordsLeft: number;
    readonly size: number;
}
declare function lowerFileTo(src: string, dest: string): Promise<LowerResult>;

declare function runShell(command: string, cwd: string): Promise<void>;
declare function buildForTarget(config: TorimiConfig, target: Target, cwd: string): Promise<string>;

interface DevOptions {
    /** ポート上書き（既定はターゲット既定ポート、env TORIMI_DEV_PORT で渡す）。 */
    readonly port?: number;
}
declare function dev(config: TorimiConfig, target: Target, cwd: string, options?: DevOptions): Promise<void>;

export { DEFAULT_TARGET, DEFAULT_WATCH_DIR, type DevOptions, type LowerResult, NATIVE_DEV_PORT, REBUILD_DEBOUNCE_MS, TARGETS, type Target, type TorimiConfig, WEB_DEV_PORT, buildForTarget, countClassKeywords, dev, findConfigPath, loadTorimiConfig, lowerFileTo, lowerForHermes, loweredBundlePath, normalizeConfig, portForTarget, resolveTarget, runShell };
