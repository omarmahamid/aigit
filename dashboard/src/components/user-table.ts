import { applyShadowStyles, el } from "../ui/shadow.js";
import { store } from "../core/store.js";
import type { UserRow } from "../core/types.js";

const css = `
:host { display:block; }
.card {
  background: linear-gradient(180deg, rgba(255,255,255,0.03), rgba(255,255,255,0.012));
  border: 1px solid var(--border);
  border-radius: var(--radius);
  box-shadow: var(--shadow);
  overflow: hidden;
}
.head {
  padding: 14px 16px;
  display:flex;
  align-items:baseline;
  justify-content:space-between;
  border-bottom: 1px solid rgba(255,255,255,0.06);
}
.title { color: var(--muted); font-size: 12px; letter-spacing: 0.06em; text-transform: uppercase; }
.meta { color: var(--muted-2); font-size: 12px; }
table { width:100%; border-collapse: collapse; font-size: 13px; }
th, td { padding: 12px 14px; border-bottom: 1px solid rgba(255,255,255,0.06); }
th { text-align:left; color: var(--muted); font-weight: 600; font-size: 12px; }
tr { cursor: pointer; }
tr:hover td { background: rgba(255,255,255,0.03); }
tr.selected td { background: rgba(124,92,255,0.12); }
.pill {
  display:inline-flex; align-items:center; gap:8px;
  padding: 4px 10px;
  border-radius: 999px;
  border: 1px solid rgba(255,255,255,0.10);
  background: rgba(0,0,0,0.12);
  font-size: 12px;
  color: var(--muted);
}
.score { color: rgba(255,255,255,0.88); font-variant-numeric: tabular-nums; }
.muted { color: var(--muted-2); }
`;

function trunc(s: string, max: number): string {
  return s.length <= max ? s : s.slice(0, Math.max(0, max - 1)) + "…";
}

export class DdUserTable extends HTMLElement {
  #shadow = this.attachShadow({ mode: "open" });
  #tbody = el("tbody");
  #meta = el("div", { class: "meta" });
  #users: UserRow[] = [];

  connectedCallback() {
    applyShadowStyles(this.#shadow, css);
    const table = el("table", {}, [
      el("thead", {}, [
        el("tr", {}, [
          el("th", {}, ["User"]),
          el("th", {}, ["Email"]),
          el("th", {}, ["Pass/Fail"]),
          el("th", {}, ["Avg score"]),
          el("th", {}, ["Last seen"]),
        ]),
      ]),
      this.#tbody,
    ]);

    this.#shadow.append(
      el("div", { class: "card" }, [el("div", { class: "head" }, [el("div", { class: "title" }, ["Users"]), this.#meta]), table])
    );

    store.addEventListener("change", () => this.render());
    this.render();
  }

  set users(v: UserRow[]) {
    this.#users = v;
    this.render();
  }

  render() {
    const s = store.getState();
    this.#meta.textContent = `${this.#users.length} users`;

    while (this.#tbody.firstChild) this.#tbody.removeChild(this.#tbody.firstChild);
    for (const u of this.#users) {
      const tr = el("tr");
      if (s.selectedEmail === u.email) tr.classList.add("selected");
      tr.addEventListener("click", () => store.setState({ selectedEmail: u.email, selectedCommit: null }));
      tr.append(
        el("td", {}, [trunc(u.name, 26)]),
        el("td", { class: "muted" }, [trunc(u.email, 34)]),
        el("td", {}, [el("span", { class: "pill" }, [`${u.passes} pass`, " / ", `${u.fails} fail`])]),
        el("td", { class: "score" }, [u.avgScore.toFixed(2)]),
        el("td", { class: "muted" }, [u.lastSeenIso])
      );
      this.#tbody.appendChild(tr);
    }
  }
}

customElements.define("dd-user-table", DdUserTable);

