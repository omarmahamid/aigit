import type { DashboardData } from "./types.js";

export type LoadStatus = "idle" | "loading" | "ready" | "error";

export type AppState = {
  status: LoadStatus;
  error: string | null;
  data: DashboardData | null;
  userFilter: string;
  selectedEmail: string | null;
  selectedCommit: string | null;
  showAnswers: boolean;
};

export class Store extends EventTarget {
  #state: AppState;

  constructor(initial: AppState) {
    super();
    this.#state = initial;
  }

  getState(): AppState {
    return this.#state;
  }

  setState(patch: Partial<AppState>) {
    this.#state = { ...this.#state, ...patch };
    this.dispatchEvent(new Event("change"));
  }

  setData(data: DashboardData) {
    this.setState({ data, status: "ready", error: null });
  }

  setError(message: string) {
    this.setState({ status: "error", error: message });
  }
}

export const store = new Store({
  status: "idle",
  error: null,
  data: null,
  userFilter: "",
  selectedEmail: null,
  selectedCommit: null,
  showAnswers: false,
});

