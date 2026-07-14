/**
 * Web entry が Hayate を使わず DOM renderer へ退避すべきかだけを判定する。
 *
 * `renderer` の backend 語彙と選択順は Hayate Web Host の責務。この関数は
 * 明示的な `dom` と、Canvas 入力に必要な EditContext の有無しか知らない。
 */
export function shouldUseDomRenderer(
  search: string,
  env: { hasEditContext: boolean },
): boolean {
  return new URLSearchParams(search).get('renderer') === 'dom' || !env.hasEditContext;
}
