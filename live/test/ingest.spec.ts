import { env } from "cloudflare:test";
import { beforeEach, describe, expect, it } from "vitest";
import { resetTokenCache } from "../worker/auth";
import { ingestLimiter, registrationLimiter } from "../worker/ratelimit";
import worker from "../worker/index";
import basic from "./fixtures/batch-basic.json";

const BASE = "https://live.test";

type IncomingRequest = Request<unknown, IncomingRequestCfProperties>;

beforeEach(() => {
  resetTokenCache();
  registrationLimiter.reset();
  ingestLimiter.reset();
});

async function call(request: Request, testEnv: Env = env): Promise<Response> {
  return worker.fetch(request as IncomingRequest, testEnv);
}

async function registerFeed(): Promise<{ id: string; token: string; url: string }> {
  const response = await call(new Request(`${BASE}/v1/feeds`, { method: "POST" }));
  expect(response.status).toBe(201);
  const body = (await response.json()) as { feed: { id: string; token: string; url: string } };
  return body.feed;
}

function ingestRequest(token: string, body: unknown): Request {
  return new Request(`${BASE}/v1/events`, {
    method: "POST",
    headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
}

describe("registration", () => {
  it("mints a feed with a slug, a token, and a live link", async () => {
    const feed = await registerFeed();
    expect(feed.id).toMatch(/^[a-z2-7]{13}$/);
    expect(feed.token).toMatch(/^[0-9a-f]{64}$/);
    expect(feed.url).toBe(`${BASE}/${feed.id}`);
  });

  it("closes when REGISTRATION_OPEN is off", async () => {
    const closedEnv = { ...env, REGISTRATION_OPEN: "false" } as unknown as Env;
    const response = await call(new Request(`${BASE}/v1/feeds`, { method: "POST" }), closedEnv);
    expect(response.status).toBe(403);
  });
});

describe("ingest", () => {
  it("rejects a missing or unknown bearer token", async () => {
    const missing = await call(
      new Request(`${BASE}/v1/events`, { method: "POST", body: JSON.stringify(basic) }),
    );
    expect(missing.status).toBe(401);

    const unknown = await call(ingestRequest("f".repeat(64), basic));
    expect(unknown.status).toBe(401);
  });

  it("commits a valid envelope directly", async () => {
    const feed = await registerFeed();
    const response = await call(ingestRequest(feed.token, basic));
    expect(response.status).toBe(202);
    expect(await response.json()).toEqual({ accepted: basic.events.length });
    const messages = await env.DB.prepare(
      `SELECT body FROM messages WHERE feed_id = ?1 ORDER BY id`,
    ).bind(feed.id).all<{ body: string }>();
    expect(messages.results.map((row) => row.body)).toEqual(["gl hf", "anyone up for 2v2?"]);
  });

  it("rejects malformed JSON and oversized batches", async () => {
    const feed = await registerFeed();

    const junk = await call(
      new Request(`${BASE}/v1/events`, {
        method: "POST",
        headers: { Authorization: `Bearer ${feed.token}` },
        body: "not json",
      }),
    );
    expect(junk.status).toBe(400);

    const oversized = {
      ...basic,
      events: Array.from({ length: 65 }, (_, index) => ({
        seq: index,
        ts: 1754700252000,
        kind: "session_started",
      })),
    };
    const tooMany = await call(ingestRequest(feed.token, oversized));
    expect(tooMany.status).toBe(400);

    const huge = await call(
      new Request(`${BASE}/v1/events`, {
        method: "POST",
        headers: {
          Authorization: `Bearer ${feed.token}`,
          "Content-Length": String(2 * 1024 * 1024),
        },
        body: "x",
      }),
    );
    expect(huge.status).toBe(413);
  });
});
