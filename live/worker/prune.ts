// Nightly retention. Feeds whose owners stopped sending (including orphans
// abandoned by a "new link" re-registration) age out along with their rows.

const DAY_MS = 86_400_000;
export async function prune(env: Env): Promise<void> {
  const retentionDays = parseRetention(env.RETENTION_DAYS);
  const now = Date.now();
  const cutoff = now - retentionDays * DAY_MS;

  await env.DB.batch([
    env.DB.prepare("DELETE FROM messages WHERE ts < ?1").bind(cutoff),
    env.DB.prepare(
      `DELETE FROM sessions
       WHERE last_seen_at < ?1
         AND id != COALESCE((
           SELECT latest.id FROM sessions latest
           WHERE latest.feed_id = sessions.feed_id
           ORDER BY latest.last_seen_at DESC, latest.started_at DESC LIMIT 1
         ), '')
         AND id != COALESCE((
           SELECT snapshot.id FROM sessions snapshot
           WHERE snapshot.feed_id = sessions.feed_id AND snapshot.synced_at IS NOT NULL
             AND EXISTS (
               SELECT 1 FROM channels
               WHERE channels.feed_id = snapshot.feed_id
                 AND channels.session_id = snapshot.id
                 AND channels.closed_at IS NULL
             )
           ORDER BY snapshot.synced_at DESC, snapshot.started_at DESC LIMIT 1
         ), '')`,
    ).bind(cutoff),
    env.DB.prepare("DELETE FROM feeds WHERE last_seen_at < ?1").bind(cutoff),
  ]);
  await env.DB.batch([
    env.DB.prepare(
      `DELETE FROM roster
       WHERE feed_id NOT IN (SELECT id FROM feeds)
          OR NOT EXISTS (
            SELECT 1 FROM sessions
            WHERE sessions.feed_id = roster.feed_id AND sessions.id = roster.session_id
          )`,
    ),
    env.DB.prepare(
      `DELETE FROM channels
       WHERE feed_id NOT IN (SELECT id FROM feeds)
          OR NOT EXISTS (
            SELECT 1 FROM sessions
            WHERE sessions.feed_id = channels.feed_id AND sessions.id = channels.session_id
          )`,
    ),
    env.DB.prepare("DELETE FROM sessions WHERE feed_id NOT IN (SELECT id FROM feeds)"),
  ]);
}

function parseRetention(value: string | undefined): number {
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) && parsed >= 1 && parsed <= 365 ? parsed : 14;
}
