import { applyShadowStyles, el } from "../ui/shadow.js";
import { store } from "../core/store.js";
import { entriesForUser } from "../core/selectors.js";
import type { DashboardEntry } from "../core/types.js";

const css = `
:host { display:block; }
.drawer {
  position: fixed;
  top: 0;
  right: 0;
  height: 100%;
  width: min(560px, 92vw);
  background: rgba(9, 13, 24, 0.88);
  border-left: 1px solid rgba(255,255,255,0.10);
  backdrop-filter: blur(18px);
  transform: translateX(100%);
  transition: transform 160ms ease;
  z-index: 20;
  box-shadow: -12px 0 40px rgba(0,0,0,0.45);
  display:flex;
  flex-direction: column;
}
.drawer.open { transform: translateX(0); }
.head {
  padding: 18px 18px 10px;
  border-bottom: 1px solid rgba(255,255,255,0.08);
}
.title { font-weight: 800; font-size: 16px; }
.subtitle { color: var(--muted); font-size: 12px; margin-top: 6px; }
.close {
  position:absolute;
  top: 14px;
  right: 14px;
  cursor: pointer;
  border-radius: 10px;
  border: 1px solid rgba(255,255,255,0.10);
  background: rgba(255,255,255,0.03);
  color: var(--text);
  padding: 8px 10px;
  font-size: 12px;
}
.body { padding: 14px 18px 18px; overflow:auto; }
.section { margin-top: 14px; }
.sectionTitle { color: var(--muted); font-size: 12px; letter-spacing: 0.06em; text-transform: uppercase; margin-bottom: 10px; }
.list { display:flex; flex-direction: column; gap: 10px; }
.item {
  border: 1px solid rgba(255,255,255,0.08);
  border-radius: 12px;
  padding: 12px;
  background: rgba(255,255,255,0.02);
  cursor: pointer;
}
.item:hover { border-color: rgba(255,255,255,0.14); background: rgba(255,255,255,0.03); }
.item.selected { border-color: rgba(124,92,255,0.45); background: rgba(124,92,255,0.10); }
.row { display:flex; align-items:center; justify-content:space-between; gap: 12px; }
.mono { font-variant-numeric: tabular-nums; }
.muted { color: var(--muted-2); font-size: 12px; }
.pill { padding: 4px 10px; border-radius: 999px; border: 1px solid rgba(255,255,255,0.10); background: rgba(0,0,0,0.12); font-size: 12px; color: var(--muted); }
.pill.pass { border-color: rgba(20,241,149,0.25); color: rgba(20,241,149,0.95); }
.pill.fail { border-color: rgba(255,92,124,0.25); color: rgba(255,92,124,0.95); }
pre {
  white-space: pre-wrap;
  word-break: break-word;
  background: rgba(0,0,0,0.18);
  border: 1px solid rgba(255,255,255,0.08);
  border-radius: 12px;
  padding: 12px;
  font-size: 12px;
  color: rgba(255,255,255,0.88);
}
`;

function trunc(s: string, max: number): string {
  return s.length <= max ? s : s.slice(0, Math.max(0, max - 1)) + "…";
}

function decisionClass(d: string) {
  return d === "pass" ? "pass" : "fail";
}

export class DdDetailDrawer extends HTMLElement {
  #shadow = this.attachShadow({ mode: "open" });
  #drawer = el("div", { class: "drawer" });
  #title = el("div", { class: "title" });
  #subtitle = el("div", { class: "subtitle" });
  #commitList = el("div", { class: "list" });
  #detail = el("div");

  connectedCallback() {
    applyShadowStyles(this.#shadow, css);
    const close = el("button", { class: "close", type: "button" }, ["Close"]);
    close.addEventListener("click", () => store.setState({ selectedEmail: null, selectedCommit: null }));

    this.#drawer.append(
      el("div", { class: "head" }, [this.#title, this.#subtitle]),
      close,
      el("div", { class: "body" }, [
        el("div", { class: "section" }, [el("div", { class: "sectionTitle" }, ["Transcripts"]), this.#commitList]),
        el("div", { class: "section" }, [el("div", { class: "sectionTitle" }, ["Details"]), this.#detail]),
      ])
    );
    this.#shadow.append(this.#drawer);
    store.addEventListener("change", () => this.render());
    this.render();
  }

  render() {
    const s = store.getState();
    const email = s.selectedEmail;
    const entries = s.data?.entries ?? [];
    const open = Boolean(email);
    this.#drawer.classList.toggle("open", open);
    if (!email) return;

    const userEntries = entriesForUser(entries, email);
    const name = userEntries[0]?.commit.author_name ?? "Unknown";
    this.#title.textContent = `${name}`;
    this.#subtitle.textContent = `${email} • ${userEntries.length} transcripts`;

    while (this.#commitList.firstChild) this.#commitList.removeChild(this.#commitList.firstChild);
    for (const e of userEntries.slice(0, 30)) {
      const isSelected = s.selectedCommit === e.commit.sha || (!s.selectedCommit && e === userEntries[0]);
      const item = el("div", { class: "item" });
      if (isSelected) item.classList.add("selected");
      item.addEventListener("click", () => store.setState({ selectedCommit: e.commit.sha }));
      const pill = el("span", { class: `pill ${decisionClass(e.transcript.decision)}` }, [e.transcript.decision]);
      item.append(
        el("div", { class: "row" }, [
          el("div", { class: "mono" }, [trunc(e.commit.sha, 10), " • ", e.commit.author_date_iso]),
          pill,
        ]),
        el("div", { class: "muted" }, [`score ${e.transcript.score.total_score.toFixed(2)} • patch ${trunc(e.transcript.diff_fingerprint.patch_id, 12)}`]),
        el("div", { class: "muted" }, [trunc(e.commit.subject, 70)])
      );
      this.#commitList.appendChild(item);
    }

    const selected = this.pickSelected(userEntries, s.selectedCommit);
    this.renderDetail(selected, s.showAnswers);
  }

  pickSelected(userEntries: DashboardEntry[], sha: string | null): DashboardEntry | null {
    if (userEntries.length === 0) return null;
    if (!sha) return userEntries[0]!;
    return userEntries.find((e) => e.commit.sha === sha) ?? userEntries[0]!;
  }

  renderDetail(entry: DashboardEntry | null, showAnswers: boolean) {
    while (this.#detail.firstChild) this.#detail.removeChild(this.#detail.firstChild);
    if (!entry) {
      this.#detail.append(el("div", { class: "muted" }, ["No transcript selected."]));
      return;
    }

    const qById = new Map(entry.transcript.exam.questions.map((q) => [q.id, q]));
    const blocks: Node[] = [];
    for (const q of entry.transcript.score.per_question) {
      const prompt = qById.get(q.id)?.prompt ?? "";
      const header = el("div", { class: "row" }, [
        el("div", {}, [`${q.id} [${q.category}]`]),
        el("div", { class: "mono muted" }, [`${q.score.toFixed(2)} (c ${q.completeness.toFixed(2)}, s ${q.specificity.toFixed(2)})`]),
      ]);
      blocks.push(el("div", { class: "item" }, [header, el("div", { class: "muted" }, [trunc(prompt.replace(/\s+/g, " "), 220)])]));

      if (showAnswers) {
        const ans = entry.transcript.answers.answers[q.id] ?? "";
        blocks.push(el("pre", {}, [ans.trim() || "(empty / not exported)"]));
      }
    }
    this.#detail.append(...blocks);
  }
}

customElements.define("dd-detail-drawer", DdDetailDrawer);

