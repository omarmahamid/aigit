import { store } from "../core/store.js";
import { loadFromUrl } from "../core/datasource.js";
import { aggregateUsers, filterUsers, kpis, timeSeriesAvgScore } from "../core/selectors.js";
import { applyShadowStyles, el } from "../ui/shadow.js";

import "./topbar.js";
import "./kpi-card.js";
import "./line-chart.js";
import "./user-table.js";
import "./detail-drawer.js";

const css = `
:host { display:block; min-height: 100vh; }
.wrap { max-width: 1220px; margin: 0 auto; padding: 18px 18px 40px; }
.grid {
  display:grid;
  grid-template-columns: repeat(12, 1fr);
  gap: 14px;
}
.kpis { grid-column: 1 / -1; display:grid; grid-template-columns: repeat(12, 1fr); gap: 14px; }
dd-kpi-card { grid-column: span 3; }
.chart { grid-column: 1 / span 7; }
.table { grid-column: 8 / -1; }
.status {
  margin-top: 16px;
  padding: 14px;
  border-radius: var(--radius);
  border: 1px solid rgba(255,255,255,0.10);
  background: rgba(255,255,255,0.03);
  color: var(--muted);
  line-height: 1.4;
}
.status strong { color: var(--text); }
@media (max-width: 980px) {
  dd-kpi-card { grid-column: span 6; }
  .chart { grid-column: 1 / -1; }
  .table { grid-column: 1 / -1; }
}
`;

export class DdApp extends HTMLElement {
  #shadow = this.attachShadow({ mode: "open" });
  #chart = document.createElement("dd-line-chart") as any;
  #table = document.createElement("dd-user-table") as any;
  #status = el("div", { class: "status" });

  connectedCallback() {
    applyShadowStyles(this.#shadow, css);
    this.#chart.title = "Score over time";

    const kpiGrid = el("div", { class: "kpis" }, [
      el("dd-kpi-card", { label: "Transcripts", value: "—" }),
      el("dd-kpi-card", { label: "Users", value: "—" }),
      el("dd-kpi-card", { label: "Pass rate", value: "—" }),
      el("dd-kpi-card", { label: "Hallucination flags", value: "—" }),
    ]);

    const grid = el("div", { class: "grid" }, [
      kpiGrid,
      el("div", { class: "chart" }, [this.#chart]),
      el("div", { class: "table" }, [this.#table]),
    ]);

    this.#shadow.append(el("dd-topbar"), el("div", { class: "wrap" }, [grid, this.#status]), el("dd-detail-drawer"));

    store.addEventListener("change", () => this.render());
    this.bootstrap();
    this.render();
  }

  async bootstrap() {
    store.setState({ status: "loading", error: null });
    try {
      const data = await loadFromUrl("./data.json");
      store.setData(data);
    } catch (e) {
      store.setState({ status: "idle" });
      store.setState({
        error:
          "No ./data.json found. Generate it with: `aigit dashboard export --out dashboard/public/data.json` then serve dashboard/public/.",
      });
    }
  }

  render() {
    const s = store.getState();
    const entries = s.data?.entries ?? [];
    const stats = kpis(entries);

    const cards = Array.from(this.#shadow.querySelectorAll("dd-kpi-card"));
    if (cards.length === 4) {
      (cards[0] as HTMLElement).setAttribute("value", String(stats.total));
      (cards[1] as HTMLElement).setAttribute("value", String(stats.users));
      (cards[2] as HTMLElement).setAttribute("value", `${(stats.passRate * 100).toFixed(1)}%`);
      (cards[2] as HTMLElement).setAttribute("hint", `${stats.pass} pass • ${stats.fail} fail`);
      (cards[3] as HTMLElement).setAttribute("value", String(stats.flags));
    }

    const users = filterUsers(aggregateUsers(entries), s.userFilter);
    this.#table.users = users;
    this.#chart.points = timeSeriesAvgScore(entries);

    const statusLines: string[] = [];
    if (s.status === "loading") statusLines.push("Loading…");
    if (s.error) statusLines.push(s.error);
    if (s.data) statusLines.push(`Generated at: ${s.data.generated_at}`);

    this.#status.innerHTML = statusLines.length
      ? `<strong>Status</strong><br/>${statusLines.map((l) => escapeHtml(l)).join("<br/>")}`
      : "";
    this.#status.style.display = statusLines.length ? "block" : "none";
  }
}

function escapeHtml(s: string): string {
  return s.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}

customElements.define("dd-app", DdApp);

