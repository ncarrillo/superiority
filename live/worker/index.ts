// superiority live worker: ingest + read api + gpui wasm assets.
// static assets handle everything outside /v1/* (run_worker_first in
// wrangler.jsonc), including the /f/<feed> viewer fallback.

import { handleRegister } from "./feeds";
import { handleIngest } from "./ingest";
import { prune } from "./prune";
import { handleRead } from "./read";

export default {
  async fetch(request, env): Promise<Response> {
    const url = new URL(request.url);
    const path = url.pathname;

    if (path === "/v1/feeds") {
      if (request.method !== "POST") return methodNotAllowed("POST");
      return handleRegister(request, env);
    }
    if (path === "/v1/events") {
      if (request.method !== "POST") return methodNotAllowed("POST");
      return handleIngest(request, env);
    }
    if (path.startsWith("/v1/")) {
      const publicRead = path.startsWith("/v1/feeds/");
      if (request.method === "OPTIONS" && publicRead) return readPreflight();
      if (request.method !== "GET") return methodNotAllowed("GET");
      const response = await handleRead(request, env);
      const result = response ?? json(404, { error: "unknown route" });
      return publicRead ? withReadCors(result) : result;
    }
    // non-/v1 paths only reach the worker when no asset matches and the viewer
    // fallback is exhausted; answer plainly.
    return json(404, { error: "not found" });
  },

  async scheduled(_controller, env): Promise<void> {
    await prune(env);
  },
} satisfies ExportedHandler<Env>;

function methodNotAllowed(allow: string): Response {
  return new Response(JSON.stringify({ error: "method not allowed" }), {
    status: 405,
    headers: { "Content-Type": "application/json; charset=utf-8", Allow: allow },
  });
}

function json(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json; charset=utf-8", "Cache-Control": "no-store" },
  });
}

function readPreflight(): Response {
  return new Response(null, {
    status: 204,
    headers: {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Methods": "GET, OPTIONS",
      "Access-Control-Allow-Headers": "Accept",
      "Access-Control-Max-Age": "86400",
    },
  });
}

function withReadCors(response: Response): Response {
  const result = new Response(response.body, response);
  result.headers.set("Access-Control-Allow-Origin", "*");
  return result;
}
