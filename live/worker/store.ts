import {
  channelKind,
  type Envelope,
  type WireChannel,
  type WireEvent,
  type WireSession,
  type WireUser,
  userField,
} from "./events";

export async function storeEnvelope(db: D1Database, feedId: string, envelope: Envelope): Promise<void> {
  const prior = await db
    .prepare(`SELECT last_seq FROM sessions WHERE feed_id = ?1 AND id = ?2`)
    .bind(feedId, envelope.session.id)
    .first<{ last_seq: number }>();
  const lastSeq = prior?.last_seq ?? -1;
  const events = envelope.events.filter((event) => event.seq > lastSeq);
  if (events.length === 0) return;

  const newest = events[events.length - 1] as WireEvent;
  const receivedAt = Date.now();
  const statements: D1PreparedStatement[] = [
    sessionUpkeep(db, feedId, envelope.session, newest, receivedAt),
    feedUpkeep(db, feedId, receivedAt),
  ];
  for (const event of events) {
    statements.push(...statementsFor(db, feedId, envelope.session, event, receivedAt));
  }
  await db.batch(statements);
}

function statementsFor(
  db: D1Database,
  feedId: string,
  session: WireSession,
  event: WireEvent,
  receivedAt: number,
): D1PreparedStatement[] {
  const statements: D1PreparedStatement[] = [];
  const channel = event.channel;
  if (channel !== undefined && typeof channel.key === "string") {
    statements.push(channelTouch(db, feedId, session, channel, event));
  }

  switch (event.kind) {
    case "session_started":
      statements.push(
        db
          .prepare(
            `UPDATE sessions SET ended_at = NULL, synced_at = NULL, sync_seq = 0
             WHERE feed_id = ?1 AND id = ?2`,
          )
          .bind(feedId, session.id),
      );
      break;
    case "heartbeat":
    case "joined":
      break;
    case "sync_started":
      statements.push(
        db
          .prepare(
            `UPDATE sessions SET sync_seq = ?3
             WHERE feed_id = ?1 AND id = ?2 AND ended_at IS NULL`,
          )
          .bind(feedId, session.id, event.seq),
      );
      break;
    case "session_synced":
      statements.push(
        db
          .prepare(
            `UPDATE channels SET closed_at = ?3
             WHERE feed_id = ?1 AND session_id = ?2 AND closed_at IS NULL
               AND last_seq < COALESCE((
                 SELECT sync_seq FROM sessions WHERE feed_id = ?1 AND id = ?2
               ), 0)`,
          )
          .bind(feedId, session.id, receivedAt),
        db
          .prepare(
            `UPDATE sessions SET synced_at = ?3
             WHERE feed_id = ?1 AND id = ?2 AND ended_at IS NULL`,
          )
          .bind(feedId, session.id, event.ts),
      );
      break;
    case "session_ended":
      break;
    case "dropped":
      break;
    case "left":
      statements.push(...leftStatements(db, feedId, session, event));
      break;
    case "message":
      statements.push(...messageStatements(db, feedId, session, event));
      break;
    case "notice":
      statements.push(...noticeStatements(db, feedId, session, event));
      break;
    case "member_joined":
    case "member_left":
      statements.push(...memberStatements(db, feedId, session, event));
      break;
    case "roster":
      statements.push(...rosterStatements(db, feedId, session, event));
      break;
    case "roster_delta":
      statements.push(...rosterDeltaStatements(db, feedId, session, event));
      break;
  }
  return statements;
}

function sessionUpkeep(
  db: D1Database,
  feedId: string,
  session: WireSession,
  event: WireEvent,
  lastSeenAt: number,
): D1PreparedStatement {
  return db
    .prepare(
      `INSERT INTO sessions
         (feed_id, id, product, client_version, started_at, last_seen_at, last_seq)
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
       ON CONFLICT(feed_id, id) DO UPDATE SET
         product = excluded.product,
         last_seen_at = max(last_seen_at, excluded.last_seen_at),
         last_seq = max(last_seq, excluded.last_seq),
         ended_at = CASE
           WHEN excluded.last_seq > last_seq AND ?8 = 'session_ended' THEN excluded.last_seen_at
           ELSE ended_at
         END`,
    )
    .bind(
      feedId,
      session.id,
      session.product,
      session.client_version,
      session.started_at,
      lastSeenAt,
      event.seq,
      event.kind,
    );
}

function feedUpkeep(db: D1Database, feedId: string, ts: number): D1PreparedStatement {
  return db.prepare(`UPDATE feeds SET last_seen_at = max(last_seen_at, ?2) WHERE id = ?1`).bind(feedId, ts);
}

function channelTouch(
  db: D1Database,
  feedId: string,
  session: WireSession,
  channel: WireChannel,
  event: WireEvent,
): D1PreparedStatement {
  return db
    .prepare(
      `INSERT INTO channels
         (feed_id, session_id, key, kind, name, first_seen_at, last_event_at, closed_at, last_seq)
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, NULL, ?7)
       ON CONFLICT(feed_id, session_id, key) DO UPDATE SET
         name = CASE WHEN excluded.name != '' THEN excluded.name ELSE channels.name END,
         last_event_at = max(channels.last_event_at, excluded.last_event_at),
         closed_at = NULL,
         last_seq = max(channels.last_seq, excluded.last_seq)`,
    )
    .bind(
      feedId,
      session.id,
      channel.key,
      channelKind(channel.key),
      channel.name ?? "",
      event.ts,
      event.seq,
    );
}

function leftStatements(
  db: D1Database,
  feedId: string,
  session: WireSession,
  event: WireEvent,
): D1PreparedStatement[] {
  const channel = requireChannel(event);
  if (channel === null) return [];
  return [
    db
      .prepare(`DELETE FROM roster WHERE feed_id = ?1 AND session_id = ?2 AND channel_key = ?3`)
      .bind(feedId, session.id, channel.key),
    db
      .prepare(
        `UPDATE channels SET closed_at = ?4, last_event_at = max(last_event_at, ?4), last_seq = ?5
         WHERE feed_id = ?1 AND session_id = ?2 AND key = ?3`,
      )
      .bind(feedId, session.id, channel.key, event.ts, event.seq),
  ];
}

function messageStatements(
  db: D1Database,
  feedId: string,
  session: WireSession,
  event: WireEvent,
): D1PreparedStatement[] {
  const channel = requireChannel(event);
  if (channel === null || typeof event.body !== "string") return [];
  // Warcraft III chat has no sender yet: handle 0 + NULL name is the
  // "no sender" sentinel, turned back into null by the read api. A sender
  // that fails to parse reads the same way rather than losing the line.
  const sender = userField(event.sender);
  const kind = event.subkind === "emote" ? "emote" : "talk";
  return [transcriptRow(db, feedId, session, event, channel.key, kind, sender, event.body)];
}

function noticeStatements(
  db: D1Database,
  feedId: string,
  session: WireSession,
  event: WireEvent,
): D1PreparedStatement[] {
  const channel = requireChannel(event);
  const kind = event.subkind;
  if (channel === null || typeof event.body !== "string") return [];
  if (kind !== "broadcast" && kind !== "information") return [];
  return [transcriptRow(db, feedId, session, event, channel.key, kind, null, event.body)];
}

function memberStatements(
  db: D1Database,
  feedId: string,
  session: WireSession,
  event: WireEvent,
): D1PreparedStatement[] {
  const channel = requireChannel(event);
  const user = userField(event.user);
  if (channel === null || user === null) return [];
  // Membership changes are transcript rows too (empty body, the mover as
  // sender); UNIQUE(feed_id, session_id, seq) dedupes retries as usual.
  const line = transcriptRow(db, feedId, session, event, channel.key, event.kind, user, "");
  if (event.kind === "member_left") {
    return [
      line,
      db
        .prepare(
          `DELETE FROM roster
           WHERE feed_id = ?1 AND session_id = ?2 AND channel_key = ?3 AND user_handle = ?4`,
        )
        .bind(feedId, session.id, channel.key, user.handle),
    ];
  }
  return [line, rosterUpsert(db, feedId, session, channel.key, user, event.seq, event.ts)];
}

function transcriptRow(
  db: D1Database,
  feedId: string,
  session: WireSession,
  event: WireEvent,
  channelKey: string,
  kind: string,
  sender: WireUser | null,
  body: string,
): D1PreparedStatement {
  return db
    .prepare(
      `INSERT OR IGNORE INTO messages
         (feed_id, session_id, seq, channel_key, ts, kind, sender_handle, sender_name, sender_clan, body)
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)`,
    )
    .bind(
      feedId,
      session.id,
      event.seq,
      channelKey,
      event.ts,
      kind,
      sender?.handle ?? 0,
      sender?.name ?? null,
      sender?.clan_tag ?? null,
      body,
    );
}

function rosterStatements(
  db: D1Database,
  feedId: string,
  session: WireSession,
  event: WireEvent,
): D1PreparedStatement[] {
  const channel = requireChannel(event);
  if (channel === null || !Array.isArray(event.users) || event.complete !== true) return [];
  const statements: D1PreparedStatement[] = [
    db
      .prepare(`DELETE FROM roster WHERE feed_id = ?1 AND session_id = ?2 AND channel_key = ?3`)
      .bind(feedId, session.id, channel.key),
  ];
  const users = event.users.map(userField).filter((user): user is WireUser => user !== null);
  if (users.length > 0) {
    statements.push(rosterBulkUpsert(db, feedId, session, channel.key, users, event.seq, event.ts));
  }
  return statements;
}

function rosterDeltaStatements(
  db: D1Database,
  feedId: string,
  session: WireSession,
  event: WireEvent,
): D1PreparedStatement[] {
  const channel = requireChannel(event);
  if (channel === null || !Array.isArray(event.users)) return [];
  const users = event.users.map(userField).filter((user): user is WireUser => user !== null);
  return users.length === 0
    ? []
    : [rosterBulkUpsert(db, feedId, session, channel.key, users, event.seq, event.ts)];
}

function rosterBulkUpsert(
  db: D1Database,
  feedId: string,
  session: WireSession,
  channelKey: string,
  users: WireUser[],
  seq: number,
  ts: number,
): D1PreparedStatement {
  return db
    .prepare(
      `INSERT INTO roster
         (feed_id, session_id, channel_key, user_handle, name, clan_tag, presence, last_seq, updated_at,
          portrait_table, portrait_offset, is_local, joined_order, avatar, is_operator)
       SELECT ?1, ?2, ?3,
              CAST(json_extract(value, '$.handle') AS INTEGER),
              json_extract(value, '$.name'),
              json_extract(value, '$.clan_tag'),
              COALESCE(json_extract(value, '$.presence'), ''),
              ?5, ?6,
              json_extract(value, '$.portrait.t'),
              json_extract(value, '$.portrait.o'),
              CASE WHEN json_extract(value, '$.is_local') THEN 1 ELSE 0 END,
              json_extract(value, '$.joined_order'),
              json_extract(value, '$.avatar'),
              CASE WHEN json_extract(value, '$.is_operator') THEN 1 ELSE 0 END
       FROM json_each(?4)
       WHERE EXISTS (
         SELECT 1 FROM channels
         WHERE feed_id = ?1 AND session_id = ?2 AND key = ?3 AND closed_at IS NULL
       )
       ON CONFLICT(feed_id, session_id, channel_key, user_handle) DO UPDATE SET
         name = excluded.name, clan_tag = excluded.clan_tag, presence = excluded.presence,
         last_seq = excluded.last_seq, updated_at = excluded.updated_at,
         portrait_table = COALESCE(excluded.portrait_table, roster.portrait_table),
         portrait_offset = COALESCE(excluded.portrait_offset, roster.portrait_offset),
         is_local = excluded.is_local,
         joined_order = COALESCE(roster.joined_order, excluded.joined_order),
         avatar = COALESCE(excluded.avatar, roster.avatar),
         is_operator = excluded.is_operator
       WHERE excluded.last_seq > roster.last_seq`,
    )
    .bind(feedId, session.id, channelKey, JSON.stringify(users), seq, ts);
}

function rosterUpsert(
  db: D1Database,
  feedId: string,
  session: WireSession,
  channelKey: string,
  user: WireUser,
  seq: number,
  ts: number,
): D1PreparedStatement {
  return db
    .prepare(
      `INSERT INTO roster
         (feed_id, session_id, channel_key, user_handle, name, clan_tag, presence, last_seq, updated_at,
          portrait_table, portrait_offset, is_local, joined_order, avatar, is_operator)
       SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
       WHERE EXISTS (
         SELECT 1 FROM channels
         WHERE feed_id = ?1 AND session_id = ?2 AND key = ?3 AND closed_at IS NULL
       )
       ON CONFLICT(feed_id, session_id, channel_key, user_handle) DO UPDATE SET
         name = excluded.name, clan_tag = excluded.clan_tag, presence = excluded.presence,
         last_seq = excluded.last_seq, updated_at = excluded.updated_at,
         portrait_table = COALESCE(excluded.portrait_table, roster.portrait_table),
         portrait_offset = COALESCE(excluded.portrait_offset, roster.portrait_offset),
         is_local = excluded.is_local,
         joined_order = COALESCE(roster.joined_order, excluded.joined_order),
         avatar = COALESCE(excluded.avatar, roster.avatar),
         is_operator = excluded.is_operator
       WHERE excluded.last_seq > roster.last_seq`,
    )
    .bind(
      feedId,
      session.id,
      channelKey,
      user.handle,
      user.name ?? null,
      user.clan_tag ?? null,
      user.presence ?? "",
      seq,
      ts,
      user.portrait?.t ?? null,
      user.portrait?.o ?? null,
      user.is_local === true ? 1 : 0,
      user.joined_order ?? null,
      user.avatar ?? null,
      user.is_operator === true ? 1 : 0,
    );
}

function requireChannel(event: WireEvent): WireChannel | null {
  const channel = event.channel;
  if (channel === undefined || typeof channel.key !== "string") return null;
  return channel;
}
