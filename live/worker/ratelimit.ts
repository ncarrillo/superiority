// Best-effort fixed-window rate limiting, in isolate memory. Good enough to
// blunt abuse of an obscure endpoint; not a distributed guarantee (each
// isolate counts separately, and counts reset on eviction).

export class RateLimiter {
  private readonly windows = new Map<string, { count: number; resetAt: number }>();

  constructor(
    private readonly limit: number,
    private readonly windowMs: number,
  ) {}

  /** Returns true when the caller is within its budget. */
  allow(key: string): boolean {
    const now = Date.now();
    const window = this.windows.get(key);
    if (window === undefined || window.resetAt <= now) {
      if (this.windows.size > 4096) this.windows.clear();
      this.windows.set(key, { count: 1, resetAt: now + this.windowMs });
      return true;
    }
    window.count += 1;
    return window.count <= this.limit;
  }

  reset(): void {
    this.windows.clear();
  }
}

/** Registrations per IP: 5 per hour. */
export const registrationLimiter = new RateLimiter(5, 3_600_000);

/** Ingest POSTs per feed: 120 per minute. */
export const ingestLimiter = new RateLimiter(120, 60_000);
