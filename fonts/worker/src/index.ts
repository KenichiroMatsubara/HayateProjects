export interface Env {
  FONTS_BUCKET: R2Bucket;
}

const CONTENT_TYPES: Record<string, string> = {
  ttf: "font/ttf",
  otf: "font/otf",
  txt: "text/plain; charset=utf-8",
};

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const key = new URL(request.url).pathname.replace(/^\/+/, "");
    if (!key) {
      return new Response("Not found", { status: 404 });
    }

    const object = await env.FONTS_BUCKET.get(key);
    if (!object) {
      return new Response("Not found", { status: 404 });
    }

    const ext = key.split(".").pop() ?? "";
    const isFont = ext === "ttf" || ext === "otf";

    const headers = new Headers();
    headers.set("Content-Type", CONTENT_TYPES[ext] ?? "application/octet-stream");
    headers.set("Access-Control-Allow-Origin", "*");
    headers.set(
      "Cache-Control",
      isFont ? "public, max-age=31536000, immutable" : "public, max-age=86400",
    );
    headers.set("ETag", object.httpEtag);

    return new Response(object.body, { headers });
  },
};
