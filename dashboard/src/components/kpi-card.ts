import { applyShadowStyles, el } from "../ui/shadow.js";

const css = `
:host { display: block; }
.card {
  background: linear-gradient(180deg, rgba(255,255,255,0.05), rgba(255,255,255,0.02));
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 14px 14px 12px;
  box-shadow: var(--shadow);
  min-height: 92px;
}
.top { display:flex; align-items:center; justify-content:space-between; gap:12px; }
.label { color: var(--muted); font-size: 12px; letter-spacing: 0.06em; text-transform: uppercase; }
.value { font-size: 26px; font-weight: 700; margin-top: 10px; }
.hint { color: var(--muted-2); font-size: 12px; margin-top: 6px; }
.badge {
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 4px 10px;
  font-size: 12px;
  color: var(--muted);
  background: rgba(0,0,0,0.15);
}
`;

export class DdKpiCard extends HTMLElement {
  #shadow = this.attachShadow({ mode: "open" });
  #label = el("div", { class: "label" });
  #value = el("div", { class: "value" });
  #hint = el("div", { class: "hint" });
  #badge = el("div", { class: "badge" });

  connectedCallback() {
    applyShadowStyles(this.#shadow, css);
    const card = el("div", { class: "card" }, [
      el("div", { class: "top" }, [this.#label, this.#badge]),
      this.#value,
      this.#hint,
    ]);
    this.#shadow.append(card);
    this.render();
  }

  static get observedAttributes() {
    return ["label", "value", "hint", "badge"];
  }

  attributeChangedCallback() {
    this.render();
  }

  render() {
    this.#label.textContent = this.getAttribute("label") ?? "";
    this.#value.textContent = this.getAttribute("value") ?? "—";
    this.#hint.textContent = this.getAttribute("hint") ?? "";
    const badge = this.getAttribute("badge") ?? "";
    this.#badge.textContent = badge;
    this.#badge.style.display = badge ? "inline-flex" : "none";
  }
}

customElements.define("dd-kpi-card", DdKpiCard);

