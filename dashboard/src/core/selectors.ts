import type { DashboardEntry, UserRow } from "./types.js";

export function toMs(iso: string): number {
  const ms = Date.parse(iso);
  return Number.isFinite(ms) ? ms : 0;
}

export function aggregateUsers(entries: DashboardEntry[]): UserRow[] {
  const byEmail = new Map<string, UserRow>();
  for (const e of entries) {
    const email = e.commit.author_email;
    const existing = byEmail.get(email);
    const row =
      existing ??
      ({
        name: e.commit.author_name,
        email,
        passes: 0,
        fails: 0,
        avgScore: 0,
        lastSeenIso: e.commit.author_date_iso,
      } satisfies UserRow);

    if (e.transcript.decision === "pass") row.passes += 1;
    else row.fails += 1;

    const n = row.passes + row.fails;
    row.avgScore = (row.avgScore * (n - 1) + e.transcript.score.total_score) / n;
    if (e.commit.author_date_iso > row.lastSeenIso) row.lastSeenIso = e.commit.author_date_iso;

    if (!existing) byEmail.set(email, row);
  }
  return [...byEmail.values()].sort((a, b) => {
    const at = toMs(a.lastSeenIso);
    const bt = toMs(b.lastSeenIso);
    if (bt !== at) return bt - at;
    return a.email.localeCompare(b.email);
  });
}

export function filterUsers(users: UserRow[], query: string): UserRow[] {
  const q = query.trim().toLowerCase();
  if (!q) return users;
  return users.filter((u) => u.name.toLowerCase().includes(q) || u.email.toLowerCase().includes(q));
}

export function entriesForUser(entries: DashboardEntry[], email: string): DashboardEntry[] {
  return entries
    .filter((e) => e.commit.author_email === email)
    .slice()
    .sort((a, b) => (a.commit.author_date_iso < b.commit.author_date_iso ? 1 : -1));
}

export function kpis(entries: DashboardEntry[]) {
  const users = new Set(entries.map((e) => e.commit.author_email));
  const total = entries.length;
  const pass = entries.filter((e) => e.transcript.decision === "pass").length;
  const fail = total - pass;
  const avgScore = total === 0 ? 0 : entries.reduce((acc, e) => acc + e.transcript.score.total_score, 0) / total;
  const flags = entries.reduce((acc, e) => acc + (e.transcript.score.hallucination_flags?.length ?? 0), 0);
  return {
    total,
    users: users.size,
    pass,
    fail,
    passRate: total === 0 ? 0 : pass / total,
    avgScore,
    flags,
  };
}

export type Point = { x: number; y: number };

export function timeSeriesAvgScore(entries: DashboardEntry[]): Point[] {
  const sorted = entries
    .slice()
    .sort((a, b) => (a.commit.author_date_iso < b.commit.author_date_iso ? -1 : 1));
  const out: Point[] = [];
  for (const e of sorted) {
    out.push({ x: toMs(e.commit.author_date_iso), y: e.transcript.score.total_score });
  }
  return out;
}

