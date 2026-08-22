import { env } from "cloudflare:test";
import { beforeEach, describe, expect, it } from "vitest";
import { resetTokenCache } from "../worker/auth";
import type { WireEvent, WireSession } from "../worker/events";
import { ingestLimiter, registrationLimiter } from "../worker/ratelimit";
import worker from "../worker/index";
import { storeEnvelope } from "../worker/store";
import basic from "./fixtures/batch-basic.json";
import remastered from "./fixtures/batch-remastered.json";

const BASE = "https://live.test";
const FEED = "ddddddddddddd";

type IncomingRequest = Request<unknown, IncomingRequestCfProperties>;

async function call(request: Request): Promise<Response> {
  return worker.fetch(request as IncomingRequest, env);
}

async function get(path: string): Promise<Response> {
  return call(new Request(`${BASE}${path}`));
}

async function store(session: WireSession, events: WireEvent[]): Promise<void> {
  await storeEnvelope(env.DB, FEED, { v: 1, session, events });
}

// One feed, two concurrent games: the SC2 session from batch-basic and the
// SC:R session from batch-remastered, both sharing the channel-key namespace.
beforeEach(async () => {
  resetTokenCache();
  registrationLimiter.reset();
  ingestLimiter.reset();
  const now = Date.now();
  await env.DB.prepare(
    `INSERT INTO feeds (id, token_hash, created_at, last_seen_at) VALUES (?1, 'hash-d', ?2, ?2)`,
  ).bind(FEED, now).run();
  await store(basic.session as WireSession, basic.events as WireEvent[]);
  await store(remastered.session as WireSession, remastered.events as WireEvent[]);
});

describe("product routes", () => {
  it("lists every product in the overview while the legacy view keeps one snapshot", async () => {
    const body = (await (await get(`/v1/feeds/${FEED}/overview`)).json()) as {
      status: { product: string; state: string; session: { id: string } };
      channels: Array<{ key: string }>;
      products: Array<{
        product: string;
        status: { product: string; state: string; session: { id: string } | null };
        channels: Array<{ key: string; member_count: number }>;
      }>;
    };
    // legacy behaviour: one snapshot across every product (scr synced last).
    expect(body.status.product).toBe("scr");
    expect(body.status.session.id).toBe(remastered.session.id);
    expect(body.channels.map((channel) => channel.key)).toEqual(["public:9"]);
    // product view: each game keeps its own snapshot and channels.
    expect(body.products.map((entry) => entry.product)).toEqual(["sc2", "scr"]);
    const [sc2, scr] = body.products;
    expect(sc2?.status).toMatchObject({ product: "sc2", state: "online" });
    expect(sc2?.status.session?.id).toBe(basic.session.id);
    expect(sc2?.channels).toEqual([expect.objectContaining({ key: "public:1033", member_count: 2 })]);
    expect(scr?.status).toMatchObject({ product: "scr", state: "online" });
    expect(scr?.status.session?.id).toBe(remastered.session.id);
    expect(scr?.channels).toEqual([expect.objectContaining({ key: "public:9", member_count: 2 })]);
  });

  it("serves one product's overview on its own route", async () => {
    const body = (await (await get(`/v1/feeds/${FEED}/products/sc2/overview`)).json()) as {
      status: { product: string; session: { id: string } };
      channels: Array<{ key: string }>;
    };
    expect(body.status.product).toBe("sc2");
    expect(body.status.session.id).toBe(basic.session.id);
    expect(body.channels.map((channel) => channel.key)).toEqual(["public:1033"]);
  });

  it("answers a product with no sessions as never connected", async () => {
    const body = (await (await get(`/v1/feeds/${FEED}/products/wc3/overview`)).json()) as {
      status: Record<string, unknown>;
      channels: unknown[];
    };
    expect(body.status).toMatchObject({
      product: "wc3",
      state: "never_connected",
      live: false,
      session: null,
    });
    expect(body.channels).toEqual([]);
  });

  it("rejects unknown feeds and unknown products", async () => {
    expect((await get(`/v1/feeds/zzzzzzzzzzzzz/products/scr/overview`)).status).toBe(404);
    expect((await get(`/v1/feeds/${FEED}/products/diablo/overview`)).status).toBe(404);
    expect((await get(`/v1/feeds/${FEED}/products/diablo/channels/public%3A9/messages`)).status).toBe(404);
    expect((await get(`/v1/feeds/${FEED}/products/d2/channels/public%3A9/roster`)).status).toBe(404);
  });

  it("keeps colliding channel keys apart per product", async () => {
    // The SC2 session speaks in its own public:9 — a different room from
    // SC:R's public:9, same key.
    await store(basic.session as WireSession, [
      {
        seq: 9,
        ts: 1754700320000,
        kind: "message",
        channel: { key: "public:9", name: "Channel 9" },
        sender: { handle: 7, name: "Overmind" },
        body: "sc2 nine",
      },
    ]);

    const transcript = async (product: string) =>
      ((await (
        await get(`/v1/feeds/${FEED}/products/${product}/channels/${encodeURIComponent("public:9")}/messages`)
      ).json()) as { messages: Array<{ body: string }> }).messages.map((message) => message.body);

    expect(await transcript("sc2")).toEqual(["sc2 nine"]);
    const scr = await transcript("scr");
    expect(scr).toContain("gg");
    expect(scr).not.toContain("sc2 nine");

    // the legacy route cannot tell the rooms apart; both transcripts land in it.
    const legacy = (await (
      await get(`/v1/feeds/${FEED}/channels/${encodeURIComponent("public:9")}/messages`)
    ).json()) as { messages: Array<{ body: string }> };
    expect(legacy.messages.map((message) => message.body)).toEqual(
      expect.arrayContaining(["sc2 nine", "gg"]),
    );

    // rosters scope the same way: scr owns the members, sc2's room is empty.
    const members = async (product: string) =>
      ((await (
        await get(`/v1/feeds/${FEED}/products/${product}/channels/${encodeURIComponent("public:9")}/roster`)
      ).json()) as { members: Array<{ name: string }> }).members.map((member) => member.name);
    expect(await members("scr")).toEqual(["Bisu", "Jaedong"]);
    expect(await members("sc2")).toEqual([]);
  });

  it("round-trips a remastered transcript through ingest in seq order", async () => {
    const registered = await call(new Request(`${BASE}/v1/feeds`, { method: "POST" }));
    expect(registered.status).toBe(201);
    const { feed } = (await registered.json()) as { feed: { id: string; token: string } };
    const posted = await call(
      new Request(`${BASE}/v1/events`, {
        method: "POST",
        headers: { Authorization: `Bearer ${feed.token}`, "Content-Type": "application/json" },
        body: JSON.stringify(remastered),
      }),
    );
    expect(posted.status).toBe(202);

    const body = (await (
      await get(`/v1/feeds/${feed.id}/products/scr/channels/${encodeURIComponent("public:9")}/messages`)
    ).json()) as {
      messages: Array<{
        seq: number;
        kind: string;
        body: string;
        sender: { handle: number; name: string | null; clan_tag: string | null } | null;
      }>;
    };
    expect(
      body.messages.map(({ seq, kind, body: text, sender }) => ({
        seq,
        kind,
        body: text,
        sender: sender === null ? null : sender.name,
      })),
    ).toEqual([
      { seq: 6, kind: "talk", body: "gg", sender: "Flash" },
      { seq: 7, kind: "emote", body: "bows respectfully", sender: "Bisu" },
      { seq: 8, kind: "broadcast", body: "Server maintenance in 10 minutes.", sender: null },
      { seq: 9, kind: "information", body: "Welcome to Brood War USA-9.", sender: null },
      { seq: 10, kind: "talk", body: "no sender on this one", sender: null },
      { seq: 11, kind: "member_joined", body: "", sender: "Jaedong" },
      { seq: 12, kind: "member_left", body: "", sender: "Flash" },
    ]);
    expect(body.messages[0]?.sender).toEqual({ handle: 72, name: "Flash", clan_tag: "KT" });

    const roster = (await (
      await get(`/v1/feeds/${feed.id}/products/scr/channels/${encodeURIComponent("public:9")}/roster`)
    ).json()) as {
      members: Array<{ name: string; avatar: string | null; is_operator: boolean; presence: string }>;
    };
    expect(roster.members).toEqual([
      expect.objectContaining({
        name: "Bisu",
        avatar: "protoss-praetor",
        is_operator: true,
        is_local: true,
        presence: "available",
      }),
      expect.objectContaining({
        name: "Jaedong",
        avatar: "zerg-hunter",
        is_operator: false,
        presence: "in_lobby",
      }),
    ]);
  });
});
