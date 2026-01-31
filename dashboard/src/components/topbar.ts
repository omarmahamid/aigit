import { store } from "../core/store.js";
import { loadFromFile } from "../core/datasource.js";
import { applyShadowStyles, el } from "../ui/shadow.js";

const css = `
:host { display:block; }
.bar {
  display:flex;
  align-items:center;
  justify-content:space-between;
  gap: 14px;
  padding: 18px 22px;
  position: sticky;
  top: 0;
  backdrop-filter: blur(14px);
  background: rgba(7, 11, 20, 0.55);
  border-bottom: 1px solid rgba(255,255,255,0.06);
  z-index: 10;
}
.left { display:flex; align-items:center; gap: 12px; }
.brand {
  font-weight: 800;
  letter-spacing: 0.01em;
  font-size: 14px;
  padding: 6px 10px;
  border-radius: 999px;
  border: 1px solid rgba(255,255,255,0.10);
  background: rgba(255,255,255,0.04);
}
.sub { color: var(--muted); font-size: 12px; }
.right { display:flex; align-items:center; gap: 10px; }
input[type="text"] {
  width: 340px;
  max-width: 45vw;
  padding: 10px 12px;
  border-radius: 10px;
  border: 1px solid rgba(255,255,255,0.10);
  background: rgba(255,255,255,0.03);
  color: var(--text);
  outline: none;
}
input[type="text"]:focus { border-color: rgba(124,92,255,0.55); box-shadow: 0 0 0 4px rgba(124,92,255,0.12); }
.btn {
  cursor: pointer;
  border-radius: 10px;
  border: 1px solid rgba(255,255,255,0.10);
  background: rgba(255,255,255,0.03);
  color: var(--text);
  padding: 10px 12px;
  font-size: 12px;
}
.btn:hover { border-color: rgba(255,255,255,0.18); background: rgba(255,255,255,0.05); }
.toggle { display:flex; align-items:center; gap:8px; color: var(--muted); font-size: 12px; }
.toggle input { width: 16px; height:16px; }
`;

export class DdTopbar extends HTMLElement {
  #shadow = this.attachShadow({ mode: "open" });
  #filter = el("input", { type: "text", placeholder: "Filter users (name/email)..." }) as HTMLInputElement;
  #file = el("input", { type: "file", accept: "application/json", style: "display:none" }) as HTMLInputElement;
  #loadBtn = el("button", { class: "btn", type: "button" }, ["Load JSON…"]) as HTMLButtonElement;
  #showAnswers = el("input", { type: "checkbox" }) as HTMLInputElement;
  #repo = el("div", { class: "sub" });

  connectedCallback() {
    applyShadowStyles(this.#shadow, css);

    this.#filter.addEventListener("input", () => store.setState({ userFilter: this.#filter.value }));
    this.#loadBtn.addEventListener("click", () => this.#file.click());
    this.#file.addEventListener("change", async () => {
      const file = this.#file.files?.[0];
      if (!file) return;
      store.setState({ status: "loading", error: null });
      try {
        const data = await loadFromFile(file);
        store.setData(data);
      } catch (e) {
        store.setError((e as Error).message);
      } finally {
        this.#file.value = "";
      }
    });
    this.#showAnswers.addEventListener("change", () => store.setState({ showAnswers: this.#showAnswers.checked }));

    const left = el("div", { class: "left" }, [
      el("div", { class: "brand" }, ["aigit / dashboard"]),
      this.#repo,
    ]);
    const right = el("div", { class: "right" }, [
      this.#filter,
      this.#loadBtn,
      this.#file,
      el("label", { class: "toggle" }, [this.#showAnswers, el("span", {}, ["Show answers"])]),
    ]);

    this.#shadow.append(el("div", { class: "bar" }, [left, right]));

    store.addEventListener("change", () => this.render());
    this.render();
  }

  render() {
    const s = store.getState();
    const repoId = s.data?.repo_id ?? "no data (export required)";
    this.#repo.textContent = `repo: ${repoId}`;
    this.#filter.value = s.userFilter;
    this.#showAnswers.checked = s.showAnswers;
  }
}

customElements.define("dd-topbar", DdTopbar);

