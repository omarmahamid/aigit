export function applyShadowStyles(shadow: ShadowRoot, css: string) {
  const supportsAdopted =
    "adoptedStyleSheets" in Document.prototype && "replaceSync" in (globalThis as any).CSSStyleSheet?.prototype;

  if (supportsAdopted) {
    const sheet = new CSSStyleSheet();
    sheet.replaceSync(css);
    (shadow as any).adoptedStyleSheets = [...((shadow as any).adoptedStyleSheets ?? []), sheet];
    return;
  }

  const style = document.createElement("style");
  style.textContent = css;
  shadow.appendChild(style);
}

export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs: Record<string, string> = {},
  children: Array<Node | string> = []
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, v);
  for (const c of children) node.append(c);
  return node;
}

