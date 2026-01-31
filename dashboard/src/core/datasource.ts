import type { DashboardData } from "./types.js";

export class DashboardDataError extends Error {
  name = "DashboardDataError";
}

function isObject(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null;
}

export function validateDashboardData(raw: unknown): DashboardData {
  if (!isObject(raw)) throw new DashboardDataError("data.json: expected object");
  if (!("schema_version" in raw)) throw new DashboardDataError("data.json: missing schema_version");
  if (!("entries" in raw)) throw new DashboardDataError("data.json: missing entries");
  const entries = (raw as any).entries;
  if (!Array.isArray(entries)) throw new DashboardDataError("data.json: entries must be an array");
  return raw as DashboardData;
}

export async function loadFromUrl(url: string): Promise<DashboardData> {
  const res = await fetch(url, { cache: "no-store" });
  if (!res.ok) {
    throw new DashboardDataError(`failed to fetch ${url}: ${res.status} ${res.statusText}`);
  }
  const raw = (await res.json()) as unknown;
  return validateDashboardData(raw);
}

export async function loadFromFile(file: File): Promise<DashboardData> {
  const text = await file.text();
  let raw: unknown;
  try {
    raw = JSON.parse(text) as unknown;
  } catch (e) {
    throw new DashboardDataError(`invalid JSON: ${(e as Error).message}`);
  }
  return validateDashboardData(raw);
}

