import { env } from "cloudflare:test";
import { beforeEach, describe, expect, it } from "vitest";
import type { Envelope, WireEvent, WireSession } from "../worker/events";
import { storeEnvelope } from "../worker/store";

const FEED_A = "aaaaaaaaaaaaa";
const FEED_B = "bbbbbbbbbbbbb";
const session: WireSession = {
  id: "ffeeddccbbaa99887766554433221100",
  client_version: "0.1.23",
  started_at: 1_754_700_100_000,
};

beforeEach(async () => {
  const now = Date.now();
  for (const [id, hash] of [[FEED_A, "hash-a"], [FEED_B, "hash-b"]] as const) {
    await env.DB.prepare(
      `INSERT INTO feeds (id, token_hash, created_at, last_seen_at) VALUES (?1, ?2, ?3, ?3)`,
    ).bind(id, hash, now).run();
  }
});

async function store(feedId: string, events: WireEvent[], wireSession = session): Promise<void> {
  await storeEnvelope(env.DB, feedId, { v: 1, session: wireSession, events } satisfies Envelope);
}

async function names(feedId = FEED_A): Promise<string[]> {
  const rows = await env.DB.prepare(
    `SELECT name FROM roster
     WHERE feed_id = ?1 AND session_id = ?2 AND channel_key = 'public:1033'
     ORDER BY name`,
  ).bind(feedId, session.id).all<{ name: string }>();
  return rows.results.map((row) => row.name);
}

function snapshot(seq: number, users: Array<{ handle: number; name: string }>): WireEvent[] {
  const channel = { key: "public:1033", name: "General" };
  return [
    { seq, ts: 1_754_700_100_000 + seq, kind: "sync_started" },
    { seq: seq + 1, ts: 1_754_700_100_001 + seq, kind: "roster", channel, complete: true, users },
    { seq: seq + 2, ts: 1_754_700_100_002 + seq, kind: "session_synced" },
  ];
}

describe("ordered persistence", () => {
  it("is idempotent when an envelope is retried", async () => {
    const events = [
      { seq: 1, ts: 1_754_700_100_001, kind: "session_started" },
      ...snapshot(2, [{ handle: 1, name: "Alpha" }]),
      {
        seq: 5,
        ts: 1_754_700_100_005,
        kind: "message",
        channel: { key: "public:1033", name: "General" },
        sender: { handle: 1, name: "Alpha" },
        body: "hello",
      },
    ] satisfies WireEvent[];
    await store(FEED_A, events);
    await store(FEED_A, events);
    const row = await env.DB.prepare(`SELECT COUNT(*) AS count FROM messages WHERE feed_id = ?1`)
      .bind(FEED_A).first<{ count: number }>();
    expect(row?.count).toBe(1);
    expect(await names()).toEqual(["Alpha"]);
  });

  it("replaces stale roster state with the next authoritative snapshot", async () => {
    await store(FEED_A, [
      { seq: 1, ts: 1_754_700_100_001, kind: "session_started" },
      ...snapshot(2, [{ handle: 1, name: "Alpha" }, { handle: 2, name: "Stale" }]),
      {
        seq: 5,
        ts: 1_754_700_100_005,
        kind: "member_joined",
        channel: { key: "public:1033", name: "General" },
        user: { handle: 3, name: "Missed Leave" },
      },
    ]);
    expect(await names()).toEqual(["Alpha", "Missed Leave", "Stale"]);

    await store(FEED_A, snapshot(6, [{ handle: 1, name: "Alpha" }]));
    expect(await names()).toEqual(["Alpha"]);
  });

  it("closes channels omitted from a completed reconciliation", async () => {
    const group = { key: "club:42", name: "Group 42" };
    await store(FEED_A, [
      { seq: 1, ts: 1_754_700_100_001, kind: "session_started" },
      { seq: 2, ts: 1_754_700_100_002, kind: "sync_started" },
      { seq: 3, ts: 1_754_700_100_003, kind: "roster", channel: { key: "public:1033", name: "General" }, complete: true, users: [] },
      { seq: 4, ts: 1_754_700_100_004, kind: "roster", channel: group, complete: true, users: [] },
      { seq: 5, ts: 1_754_700_100_005, kind: "session_synced" },
      ...snapshot(6, []),
    ]);
    const rows = await env.DB.prepare(
      `SELECT key FROM channels WHERE feed_id = ?1 AND session_id = ?2 AND closed_at IS NULL ORDER BY key`,
    ).bind(FEED_A, session.id).all<{ key: string }>();
    expect(rows.results.map((row) => row.key)).toEqual(["public:1033"]);
  });

  it("keeps feed identity in every idempotency boundary", async () => {
    const event: WireEvent = {
      seq: 1,
      ts: 1_754_700_100_001,
      kind: "message",
      channel: { key: "public:1033", name: "General" },
      sender: { handle: 1, name: "Alpha" },
      body: "same sequence",
    };
    await store(FEED_A, [event]);
    await store(FEED_B, [event]);
    const row = await env.DB.prepare(`SELECT COUNT(*) AS count FROM messages WHERE seq = 1`)
      .first<{ count: number }>();
    expect(row?.count).toBe(2);
  });

  it("never lets delayed client timestamps win session freshness", async () => {
    const current = Date.now();
    await store(FEED_A, [{ seq: 1, ts: 1, kind: "session_started" }]);
    const row = await env.DB.prepare(
      `SELECT last_seen_at FROM sessions WHERE feed_id = ?1 AND id = ?2`,
    ).bind(FEED_A, session.id).first<{ last_seen_at: number }>();
    expect(row?.last_seen_at).toBeGreaterThanOrEqual(current);
  });
});
