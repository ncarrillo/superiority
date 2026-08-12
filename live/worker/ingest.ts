// post /v1/events: authenticate, validate, and commit one ordered envelope.

import { bearerToken, feedIdForToken } from "./auth";
import { MAX_BODY_BYTES, validateEnvelope } from "./events";
import { ingestLimiter } from "./ratelimit";
import { storeEnvelope } from "./store";

export async function handleIngest(request: Request, env: Env): Promise<Response> {
  const token = bearerToken(request);
  if (token === null) return unauthorized();
  const feedId = await feedIdForToken(env.DB, token);
  if (feedId === null) return unauthorized();

  if (!ingestLimiter.allow(feedId)) {
    return json(429, { error: "slow down" });
  }

  const declared = Number(request.headers.get("Content-Length") ?? "0");
  if (declared > MAX_BODY_BYTES) return json(413, { error: "too large" });
  const bytes = await request.arrayBuffer();
  if (bytes.byteLength > MAX_BODY_BYTES) return json(413, { error: "too large" });
  const text = new TextDecoder().decode(bytes);

  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    return json(400, { error: "not JSON" });
  }
  const result = validateEnvelope(parsed);
  if (!result.ok) return json(400, { error: result.error });

  try {
    await storeEnvelope(env.DB, feedId, result.envelope);
  } catch (error) {
    console.error("store failed", { feedId, count: result.envelope.events.length, error: String(error) });
    return json(500, { error: "store failed" });
  }
  return json(202, { accepted: result.envelope.events.length });
}

function unauthorized(): Response {
  return new Response(JSON.stringify({ error: "unauthorized" }), {
    status: 401,
    headers: {
      "Content-Type": "application/json; charset=utf-8",
      "Cache-Control": "no-store",
      "WWW-Authenticate": "Bearer",
    },
  });
}

function json(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json; charset=utf-8", "Cache-Control": "no-store" },
  });
}
