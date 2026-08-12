import { env } from "cloudflare:test";
import { beforeEach, describe, expect, it } from "vitest";
import type { WireEvent, WireSession } from "../worker/events";
import worker from "../worker/index";
import { storeEnvelope } from "../worker/store";
import basic from "./fixtures/batch-basic.json";

const BASE = "https://live.test";
const FEED = "ccccccccccccc";

async function get(path: string): Promise<Response> {
  return worker.fetch(new Request(`${BASE}${path}`) as Request<unknown, IncomingRequestCfProperties>, env);
}

async function store(session: WireSession, events: WireEvent[]): Promise<void> {
  await storeEnvelope(env.DB, FEED, { v: 1, session, events });
}

beforeEach(async () => {
  const now = Date.now();
  await env.DB.prepare(
    `INSERT INTO feeds (id, token_hash, created_at, last_seen_at) VALUES (?1, 'hash-c', ?2, ?2)`,
  ).bind(FEED, now).run();
  await store(basic.session as WireSession, basic.events as WireEvent[]);
});

describe("read api", () => {
  it("returns one consistent overview", async () => {
    const response = await get(`/v1/feeds/${FEED}/overview`);
    expect(response.status).toBe(200);
    expect(response.headers.get("Cache-Control")).toBe("no-store");
    const body = (await response.json()) as {
      status: { state: string; session: { id: string } };
      channels: Array<{ key: string; member_count: number }>;
    };
    expect(body.status.session.id).toBe(basic.session.id);
    expect(body.channels).toEqual([
      expect.objectContaining({ key: "public:1033", member_count: 2 }),
    ]);
  });

  it("keeps the last confirmed snapshot while a replacement reconnects", async () => {
    const reconnecting: WireSession = {
      id: "11112222333344445555666677778888",
      client_version: "0.1.23",
      started_at: Date.now(),
    };
    await store(reconnecting, [
      { seq: 1, ts: Date.now(), kind: "session_started" },
      { seq: 2, ts: Date.now() + 1, kind: "sync_started" },
      {
        seq: 3,
        ts: Date.now() + 2,
        kind: "roster",
        channel: { key: "private:new", name: "new" },
        complete: true,
        users: [{ handle: 99, name: "Partial" }],
      },
    ]);

    const overview = (await (await get(`/v1/feeds/${FEED}/overview`)).json()) as {
      status: { state: string; session: { id: string } };
      channels: Array<{ key: string }>;
    };
    expect(overview.status.state).toBe("reconnecting");
    expect(overview.status.session.id).toBe(basic.session.id);
    expect(overview.channels.map((channel) => channel.key)).toEqual(["public:1033"]);
  });

  it("reports explicit disconnects offline without discarding the roster", async () => {
    const session = basic.session as WireSession;
    await store(session, [{ seq: 9, ts: Date.now(), kind: "session_ended" }]);
    const overview = (await (await get(`/v1/feeds/${FEED}/overview`)).json()) as {
      status: { state: string; live: boolean };
      channels: Array<{ key: string }>;
    };
    expect(overview.status).toMatchObject({ state: "offline", live: false });
    expect(overview.channels.map((channel) => channel.key)).toEqual(["public:1033"]);

    const roster = (await (await get(
      `/v1/feeds/${FEED}/channels/${encodeURIComponent("public:1033")}/roster`,
    )).json()) as { members: Array<{ name: string }> };
    expect(roster.members.map((member) => member.name)).toEqual(["Overmind", "Radiant"]);
  });

  it("preserves chat history across sessions", async () => {
    const next: WireSession = {
      id: "99992222333344445555666677778888",
      client_version: "0.1.23",
      started_at: Date.now(),
    };
    await store(next, [{
      seq: 1,
      ts: Date.now(),
      kind: "message",
      channel: { key: "public:1033", name: "General" },
      sender: { handle: 3, name: "Next" },
      body: "after reconnect",
    }]);
    const body = (await (await get(
      `/v1/feeds/${FEED}/channels/${encodeURIComponent("public:1033")}/messages`,
    )).json()) as { messages: Array<{ body: string }> };
    expect(body.messages.map((message) => message.body)).toEqual([
      "gl hf",
      "anyone up for 2v2?",
      "after reconnect",
    ]);
  });

  it("pages messages and exposes public cors", async () => {
    const first = await get(
      `/v1/feeds/${FEED}/channels/${encodeURIComponent("public:1033")}/messages?limit=1`,
    );
    const body = (await first.json()) as { messages: Array<{ body: string }>; cursor: number };
    expect(body.messages.map((message) => message.body)).toEqual(["anyone up for 2v2?"]);
    expect(first.headers.get("Access-Control-Allow-Origin")).toBe("*");

    const after = (await (await get(
      `/v1/feeds/${FEED}/channels/${encodeURIComponent("public:1033")}/messages?after=${body.cursor}`,
    )).json()) as { messages: unknown[]; cursor: number };
    expect(after.messages).toEqual([]);
    expect(after.cursor).toBe(body.cursor);
  });

  it("does not expose social or membership history", async () => {
    expect((await get(`/v1/feeds/${FEED}/whispers`)).status).toBe(404);
    expect((await get(`/v1/feeds/${FEED}/friends`)).status).toBe(404);
    expect((await get(`/v1/feeds/${FEED}/channels/public%3A1033/events`)).status).toBe(404);
  });
});
