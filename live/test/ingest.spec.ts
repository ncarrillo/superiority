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
      `SELECT kind, body FROM messages WHERE feed_id = ?1 ORDER BY id`,
    ).bind(feed.id).all<{ kind: string; body: string }>();
    expect(messages.results).toEqual([
      { kind: "talk", body: "gl hf" },
      { kind: "member_left", body: "" },
      { kind: "talk", body: "anyone up for 2v2?" },
    ]);
  });

  it("accepts legacy SC2 envelopes but rejects unknown product identities", async () => {
    const feed = await registerFeed();
    const legacy = {
      ...basic,
      session: {
        id: basic.session.id,
        client_version: basic.session.client_version,
        started_at: basic.session.started_at,
      },
    };
    expect((await call(ingestRequest(feed.token, legacy))).status).toBe(202);

    const unknownProduct = {
      ...basic,
      session: { ...basic.session, id: "11112222333344445555666677778888", product: "diablo" },
    };
    expect((await call(ingestRequest(feed.token, unknownProduct))).status).toBe(400);
  });

  it("rejects transcript rows whose subkind is missing or made up", async () => {
    const feed = await registerFeed();
    const channel = { key: "public:9", name: "Brood War USA-9" };
    const one = (event: Record<string, unknown>) => ({
      ...basic,
      session: { ...basic.session, id: "deadbeefdeadbeefdeadbeefdeadbeef", product: "scr" },
      events: [{ seq: 1, ts: 1754700400000, ...event }],
    });

    const shout = await call(ingestRequest(feed.token, one({ kind: "message", channel, body: "hi", subkind: "shout" })));
    expect(shout.status).toBe(400);
    expect(((await shout.json()) as { error: string }).error).toContain("subkind");

    const mute = await call(ingestRequest(feed.token, one({ kind: "notice", channel, body: "maintenance" })));
    expect(mute.status).toBe(400);

    const chatty = await call(ingestRequest(feed.token, one({ kind: "notice", channel, body: "x", subkind: "talk" })));
    expect(chatty.status).toBe(400);
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
