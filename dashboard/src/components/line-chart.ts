import { applyShadowStyles, el } from "../ui/shadow.js";
import type { Point } from "../core/selectors.js";

const css = `
:host { display: block; }
.card {
  background: linear-gradient(180deg, rgba(255,255,255,0.04), rgba(255,255,255,0.015));
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 14px;
  box-shadow: var(--shadow);
}
.title { color: var(--muted); font-size: 12px; letter-spacing: 0.06em; text-transform: uppercase; }
svg { width: 100%; height: 140px; margin-top: 12px; display:block; }
.axis { stroke: rgba(255,255,255,0.08); stroke-width: 1; }
.line { fill: none; stroke: url(#grad); stroke-width: 2.2; }
.area { fill: url(#fill); opacity: 0.55; }
.empty { color: var(--muted-2); font-size: 12px; margin-top: 12px; }
`;

function clamp(v: number, a: number, b: number): number {
  return Math.max(a, Math.min(b, v));
}

function pathFor(points: Point[], w: number, h: number) {
  if (points.length === 0) return { line: "", area: "" };
  const xs = points.map((p) => p.x);
  const ys = points.map((p) => p.y);
  const minX = Math.min(...xs);
  const maxX = Math.max(...xs);
  const minY = Math.min(...ys);
  const maxY = Math.max(...ys);
  const dx = Math.max(1, maxX - minX);
  const dy = Math.max(1e-6, maxY - minY);

  const toX = (x: number) => clamp(((x - minX) / dx) * w, 0, w);
  const toY = (y: number) => clamp(h - ((y - minY) / dy) * h, 0, h);

  const pts = points.map((p) => ({ x: toX(p.x), y: toY(p.y) }));
  const line = `M ${pts[0]!.x.toFixed(2)} ${pts[0]!.y.toFixed(2)} ` + pts.slice(1).map((p) => `L ${p.x.toFixed(2)} ${p.y.toFixed(2)}`).join(" ");
  const area =
    line +
    ` L ${pts[pts.length - 1]!.x.toFixed(2)} ${h.toFixed(2)} L ${pts[0]!.x.toFixed(2)} ${h.toFixed(2)} Z`;
  return { line, area };
}

export class DdLineChart extends HTMLElement {
  #shadow = this.attachShadow({ mode: "open" });
  #title = el("div", { class: "title" });
  #svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  #empty = el("div", { class: "empty" }, ["No data yet."]);
  #points: Point[] = [];

  connectedCallback() {
    applyShadowStyles(this.#shadow, css);
    const card = el("div", { class: "card" }, [this.#title, this.#svg, this.#empty]);
    this.#shadow.append(card);
    this.render();
  }

  set title(v: string) {
    this.#title.textContent = v;
  }

  set points(v: Point[]) {
    this.#points = v;
    this.render();
  }

  render() {
    this.#svg.setAttribute("viewBox", "0 0 640 140");
    while (this.#svg.firstChild) this.#svg.removeChild(this.#svg.firstChild);

    if (this.#points.length < 2) {
      this.#svg.style.display = "none";
      this.#empty.style.display = "block";
      return;
    }
    this.#svg.style.display = "block";
    this.#empty.style.display = "none";

    const defs = document.createElementNS("http://www.w3.org/2000/svg", "defs");
    defs.innerHTML = `
      <linearGradient id="grad" x1="0" y1="0" x2="1" y2="0">
        <stop offset="0" stop-color="var(--accent)"/>
        <stop offset="1" stop-color="var(--accent-2)"/>
      </linearGradient>
      <linearGradient id="fill" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stop-color="rgba(124, 92, 255, 0.25)"/>
        <stop offset="1" stop-color="rgba(20, 241, 149, 0.02)"/>
      </linearGradient>
    `;
    this.#svg.appendChild(defs);

    const axis = document.createElementNS("http://www.w3.org/2000/svg", "line");
    axis.setAttribute("x1", "0");
    axis.setAttribute("x2", "640");
    axis.setAttribute("y1", "139");
    axis.setAttribute("y2", "139");
    axis.setAttribute("class", "axis");
    this.#svg.appendChild(axis);

    const { line, area } = pathFor(this.#points, 640, 140);
    const areaPath = document.createElementNS("http://www.w3.org/2000/svg", "path");
    areaPath.setAttribute("d", area);
    areaPath.setAttribute("class", "area");
    this.#svg.appendChild(areaPath);

    const linePath = document.createElementNS("http://www.w3.org/2000/svg", "path");
    linePath.setAttribute("d", line);
    linePath.setAttribute("class", "line");
    this.#svg.appendChild(linePath);
  }
}

customElements.define("dd-line-chart", DdLineChart);

