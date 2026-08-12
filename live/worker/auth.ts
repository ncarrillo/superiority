// Feed attribution: the bearer token is the routing key. Tokens are stored
// only as SHA-256 hashes; the lookup result is cached per isolate so the hot
// ingest path usually skips D1.

const CACHE_TTL_MS = 300_000;
const tokenCache = new Map<string, { feedId: string; expires: number }>();

export async function sha256Hex(text: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

export function bearerToken(request: Request): string | null {
  const header = request.headers.get("Authorization") ?? "";
  if (!header.startsWith("Bearer ")) return null;
  const token = header.slice("Bearer ".length).trim();
  return token.length >= 1 && token.length <= 128 ? token : null;
}

/** Resolves a presented token to its feed id, or null. Revoked feeds resolve
 * to null. Only positive results are cached (a just-registered token must
 * never be locked out by a stale negative). */
export async function feedIdForToken(db: D1Database, token: string): Promise<string | null> {
  const hash = await sha256Hex(token);
  const cached = tokenCache.get(hash);
  const now = Date.now();
  if (cached !== undefined && cached.expires > now) return cached.feedId;
  tokenCache.delete(hash);

  const row = await db
    .prepare("SELECT id FROM feeds WHERE token_hash = ?1 AND revoked = 0")
    .bind(hash)
    .first<{ id: string }>();
  if (row === null) return null;
  tokenCache.set(hash, { feedId: row.id, expires: now + CACHE_TTL_MS });
  return row.id;
}

/** Test hook: the per-isolate cache outlives individual specs. */
export function resetTokenCache(): void {
  tokenCache.clear();
}
