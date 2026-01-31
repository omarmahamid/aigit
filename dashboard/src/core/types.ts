export type Decision = "pass" | "fail";

export type Transcript = {
  schema_version: string;
  commit?: string | null;
  timestamp: string;
  repo_id: string;
  repo_fingerprint: string;
  diff_fingerprint: { patch_id: string };
  exam: { questions: Array<{ id: string; category: string; prompt: string; choices?: string[] | null }> };
  answers: { answers: Record<string, string> };
  score: {
    total_score: number;
    hallucination_flags: string[];
    per_question: Array<{ id: string; category: string; score: number; completeness: number; specificity: number; notes: string[] }>;
  };
  decision: Decision;
};

export type CommitMeta = {
  sha: string;
  author_name: string;
  author_email: string;
  author_date_iso: string;
  subject: string;
};

export type DashboardEntry = {
  commit: CommitMeta;
  transcript: Transcript;
};

export type DashboardData = {
  schema_version: "aigit-dashboard/0.1" | string;
  generated_at: string;
  repo_id: string;
  entries: DashboardEntry[];
};

export type UserRow = {
  name: string;
  email: string;
  passes: number;
  fails: number;
  avgScore: number;
  lastSeenIso: string;
};

