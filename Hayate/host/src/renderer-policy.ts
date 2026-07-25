/**
 * Canvas Mode no longer exposes a runtime Scene Renderer selector. `auto` always boots the single
 * Worker bundle and its Rust Render Host owns initial renderer selection. HTML Mode remains the
 * separately authored `dom` choice in each Web entry.
 */
export const WEB_RENDERER_QUERY_VALUES = ['auto'] as const;
