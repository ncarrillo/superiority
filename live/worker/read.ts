const LIVE_WINDOW_MS = 45_000;
const DEFAULT_LIMIT = 100;
const MAX_LIMIT = 500;

// One feed now carries concurrent sessions from several games (one session
// per product). The legacy routes keep their pre-product semantics — the
// latest/snapshot session across every product — because the deployed viewer
// reads them. The /products/:product/... routes scope the same reads to one
// game; that matters for channels because keys collide across products
// (SC2 'public:9' and SC:R 'public:9' are different rooms).
const FEED_ROUTE = /^\/v1\/feeds\/([a-z2-7]{1,32})\/(status|channels|overview)$/;
const PRODUCT_ROUTE = /^\/v1\/feeds\/([a-z2-7]{1,32})\/products\/(sc2|scr|wc3)\/overview$/;
const CHANNEL_ROUTE = /^\/v1\/feeds\/([a-z2-7]{1,32})(?:\/products\/(sc2|scr|wc3))?\/channels\/([^/]+)\/(messages|roster)$/;

interface FeedRow {
  id: string;
  product: string;
}

interface SessionRow {
  id: string;
  product: string;
  client_version: string;
  started_at: number;
  last_seen_at: number;
  last_seq: number;
  synced_at: number | null;
  ended_at: number | null;
}

export async function handleRead(request: Request, env: Env): Promise<Response | null> {
  const url = new URL(request.url);
  const feedMatch = FEED_ROUTE.exec(url.pathname);
  if (feedMatch !== null) {
    const feedId = feedMatch[1] ?? "";
    if (feedMatch[2] === "status") return status(env.DB, feedId);
    if (feedMatch[2] === "channels") return channels(env.DB, feedId);
    return overview(env.DB, feedId);
  }

  const productMatch = PRODUCT_ROUTE.exec(url.pathname);
  if (productMatch !== null) {
    return productOverview(env.DB, productMatch[1] ?? "", productMatch[2] ?? "");
  }

  const channelMatch = CHANNEL_ROUTE.exec(url.pathname);
  if (channelMatch === null) return null;
  const feedId = channelMatch[1] ?? "";
  const product = channelMatch[2] ?? null;
  let channelKey: string;
  try {
    channelKey = decodeURIComponent(channelMatch[3] ?? "");
  } catch {
    return json(400, { error: "bad channel key" });
  }
  return channelMatch[4] === "messages"
    ? messages(env.DB, feedId, product, channelKey, parseAfter(url.searchParams.get("after")), parseLimit(url.searchParams.get("limit")))
    : roster(env.DB, feedId, product, channelKey);
}

// `product` scopes a query to one game's sessions; null spans them all.

async function latestSession(db: D1Database, feedId: string, product: string | null): Promise<SessionRow | null> {
  return db
    .prepare(
      `SELECT id, product, client_version, started_at, last_seen_at, last_seq, synced_at, ended_at
       FROM sessions WHERE feed_id = ?1 AND (?2 IS NULL OR product = ?2)
       ORDER BY last_seen_at DESC, started_at DESC LIMIT 1`,
    )
    .bind(feedId, product)
    .first<SessionRow>();
}

async function snapshotSession(db: D1Database, feedId: string, product: string | null): Promise<SessionRow | null> {
  return db
    .prepare(
      `SELECT id, product, client_version, started_at, last_seen_at, last_seq, synced_at, ended_at
       FROM sessions
       WHERE feed_id = ?1 AND synced_at IS NOT NULL AND (?2 IS NULL OR product = ?2)
         AND EXISTS (
           SELECT 1 FROM channels
           WHERE channels.feed_id = sessions.feed_id
             AND channels.session_id = sessions.id
             AND channels.closed_at IS NULL
         )
       ORDER BY synced_at DESC, started_at DESC LIMIT 1`,
    )
    .bind(feedId, product)
    .first<SessionRow>();
}

async function status(db: D1Database, feedId: string): Promise<Response> {
  const result = await statusData(db, feedId);
  return result === null ? json(404, { error: "unknown feed" }) : json(200, result);
}

async function statusData(db: D1Database, feedId: string): Promise<Record<string, unknown> | null> {
  const [feed, latest, snapshot] = await Promise.all([
    feedRow(db, feedId),
    latestSession(db, feedId, null),
    snapshotSession(db, feedId, null),
  ]);
  return feed === null ? null : statusOf(feed, latest, snapshot, null);
}

function statusOf(
  feed: FeedRow,
  latest: SessionRow | null,
  snapshot: SessionRow | null,
  product: string | null,
): Record<string, unknown> {
  const now = Date.now();
  const fresh = latest !== null && now - latest.last_seen_at < LIVE_WINDOW_MS;
  const state =
    latest === null
      ? "never_connected"
      : latest.ended_at !== null || !fresh
        ? "offline"
        : latest.synced_at === null
          ? "reconnecting"
          : "online";
  return {
    ok: true,
    product: product ?? snapshot?.product ?? latest?.product ?? feed.product,
    state,
    live: state === "online",
    session: snapshot,
  };
}

async function channels(db: D1Database, feedId: string): Promise<Response> {
  const [exists, rows] = await Promise.all([feedExists(db, feedId), channelRows(db, feedId, null)]);
  return exists ? json(200, { channels: rows }) : json(404, { error: "unknown feed" });
}

async function overview(db: D1Database, feedId: string): Promise<Response> {
  const [feed, latest, snapshot, channels, slugs] = await Promise.all([
    feedRow(db, feedId),
    latestSession(db, feedId, null),
    snapshotSession(db, feedId, null),
    channelRows(db, feedId, null),
    sessionProducts(db, feedId),
  ]);
  if (feed === null) return json(404, { error: "unknown feed" });
  const products = await Promise.all(
    slugs.map(async (product) => {
      const [productLatest, productSnapshot, productChannels] = await Promise.all([
        latestSession(db, feedId, product),
        snapshotSession(db, feedId, product),
        channelRows(db, feedId, product),
      ]);
      return {
        product,
        status: statusOf(feed, productLatest, productSnapshot, product),
        channels: productChannels,
      };
    }),
  );
  return json(200, { status: statusOf(feed, latest, snapshot, null), channels, products });
}

async function productOverview(db: D1Database, feedId: string, product: string): Promise<Response> {
  const [feed, latest, snapshot, channels] = await Promise.all([
    feedRow(db, feedId),
    latestSession(db, feedId, product),
    snapshotSession(db, feedId, product),
    channelRows(db, feedId, product),
  ]);
  if (feed === null) return json(404, { error: "unknown feed" });
  return json(200, { status: statusOf(feed, latest, snapshot, product), channels });
}

/** Products with at least one session on the feed, alphabetically. */
async function sessionProducts(db: D1Database, feedId: string): Promise<string[]> {
  const result = await db
    .prepare(`SELECT DISTINCT product FROM sessions WHERE feed_id = ?1 ORDER BY product`)
    .bind(feedId)
    .all<{ product: string }>();
  return result.results.map((row) => row.product);
}

async function channelRows(db: D1Database, feedId: string, product: string | null): Promise<Record<string, unknown>[]> {
  const result = await db
    .prepare(
      `SELECT c.key, c.kind, c.name, c.first_seen_at, c.last_event_at,
              (SELECT COUNT(*) FROM roster r
                WHERE r.feed_id = c.feed_id AND r.session_id = c.session_id
                  AND r.channel_key = c.key) AS member_count
       FROM channels c
       WHERE c.feed_id = ?1 AND c.closed_at IS NULL
         AND c.session_id = (
           SELECT id FROM sessions
           WHERE feed_id = ?1 AND synced_at IS NOT NULL AND (?2 IS NULL OR product = ?2)
             AND EXISTS (
               SELECT 1 FROM channels snapshot_channels
               WHERE snapshot_channels.feed_id = sessions.feed_id
                 AND snapshot_channels.session_id = sessions.id
                 AND snapshot_channels.closed_at IS NULL
             )
           ORDER BY synced_at DESC, started_at DESC LIMIT 1
         )
       ORDER BY c.first_seen_at ASC, c.key ASC`,
    )
    .bind(feedId, product)
    .all();
  return result.results;
}

async function messages(
  db: D1Database,
  feedId: string,
  product: string | null,
  channelKey: string,
  after: number | null,
  limit: number,
): Promise<Response> {
  const query = after === null
    ? `SELECT id, ts, seq, kind, sender_handle, sender_name, sender_clan, body
       FROM messages WHERE feed_id = ?1 AND channel_key = ?2
         AND (?3 IS NULL OR session_id IN (SELECT id FROM sessions WHERE feed_id = ?1 AND product = ?3))
       ORDER BY id DESC LIMIT ?4`
    : `SELECT id, ts, seq, kind, sender_handle, sender_name, sender_clan, body
       FROM messages WHERE feed_id = ?1 AND channel_key = ?2
         AND (?3 IS NULL OR session_id IN (SELECT id FROM sessions WHERE feed_id = ?1 AND product = ?3))
         AND id > ?5
       ORDER BY id ASC LIMIT ?4`;
  const prepared = db.prepare(query);
  const result = after === null
    ? await prepared.bind(feedId, channelKey, product, limit).all<Record<string, unknown>>()
    : await prepared.bind(feedId, channelKey, product, limit, after).all<Record<string, unknown>>();
  if (!(await feedExists(db, feedId))) return json(404, { error: "unknown feed" });
  const rows = after === null ? result.results.reverse() : result.results;
  const cursor = rows.length > 0 ? (rows[rows.length - 1]?.id ?? after ?? 0) : (after ?? 0);
  return json(200, {
    messages: rows.map((row) => ({
      id: row.id,
      ts: row.ts,
      seq: row.seq,
      kind: row.kind,
      // handle 0 + NULL name is the stored "no sender" sentinel.
      sender:
        row.sender_handle === 0 && row.sender_name === null
          ? null
          : { handle: row.sender_handle, name: row.sender_name, clan_tag: row.sender_clan },
      body: row.body,
    })),
    cursor,
  });
}

async function roster(
  db: D1Database,
  feedId: string,
  product: string | null,
  channelKey: string,
): Promise<Response> {
  const result = await db
    .prepare(
      `SELECT r.user_handle, r.name, r.clan_tag, r.presence, r.updated_at,
              r.is_local, r.joined_order, r.avatar, r.is_operator,
              r.portrait_table, r.portrait_offset
       FROM roster r
       WHERE r.feed_id = ?1 AND r.channel_key = ?2
         AND r.session_id = (
           SELECT id FROM sessions
           WHERE feed_id = ?1 AND synced_at IS NOT NULL AND (?3 IS NULL OR product = ?3)
             AND EXISTS (
               SELECT 1 FROM channels snapshot_channels
               WHERE snapshot_channels.feed_id = sessions.feed_id
                 AND snapshot_channels.session_id = sessions.id
                 AND snapshot_channels.closed_at IS NULL
             )
           ORDER BY synced_at DESC, started_at DESC LIMIT 1
         )
       ORDER BY r.joined_order IS NULL ASC, r.joined_order ASC, r.user_handle ASC`,
    )
    .bind(feedId, channelKey, product)
    .all();
  if (!(await feedExists(db, feedId))) return json(404, { error: "unknown feed" });
  return json(200, {
    members: result.results.map((row) => ({
      handle: row.user_handle,
      name: row.name,
      clan_tag: row.clan_tag,
      presence: row.presence,
      updated_at: row.updated_at,
      is_local: row.is_local === 1,
      is_operator: row.is_operator === 1,
      avatar: row.avatar,
      joined_order: row.joined_order,
      portrait:
        row.portrait_table === null || row.portrait_offset === null
          ? null
          : { t: row.portrait_table, o: row.portrait_offset },
    })),
  });
}

async function feedRow(db: D1Database, feedId: string): Promise<FeedRow | null> {
  return db.prepare(`SELECT id, product FROM feeds WHERE id = ?1`).bind(feedId).first<FeedRow>();
}

async function feedExists(db: D1Database, feedId: string): Promise<boolean> {
  return (await feedRow(db, feedId)) !== null;
}

function parseAfter(value: string | null): number | null {
  if (value === null) return null;
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) && parsed >= 0 ? parsed : null;
}

function parseLimit(value: string | null): number {
  if (value === null) return DEFAULT_LIMIT;
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 1) return DEFAULT_LIMIT;
  return Math.min(parsed, MAX_LIMIT);
}

function json(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json; charset=utf-8", "Cache-Control": "no-store" },
  });
}
