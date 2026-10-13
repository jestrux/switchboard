import { parseMarkdownDocs, organizeSidebar } from "@jestrux/docs-viewer/parser";
import type { DocsConfig } from "@jestrux/docs-viewer";

// Every Markdown file under ../docs with frontmatter is a page. Add a file and it shows up.
const modules = import.meta.glob("../../docs/**/*.md", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

type Page = { path: string; content: string; category?: string; id?: string };

const pages: Page[] = Object.entries(modules)
  .map(([path, content]) => {
    const front = content.match(/^---\n([\s\S]*?)\n---/)?.[1] ?? "";
    const field = (name: string) => front.match(new RegExp(`^${name}:\\s*(.+)$`, "m"))?.[1].trim();
    return { path: path.replace("../../docs/", ""), content, category: field("category"), id: field("id") };
  })
  .sort((a, b) => a.path.localeCompare(b.path));

/** Resolve `target` against the folder of `from`, both relative to docs/. */
function resolve(from: string, target: string): string {
  const parts = from.split("/").slice(0, -1);
  for (const seg of target.split("/")) {
    if (seg === "..") parts.pop();
    else if (seg !== "." && seg !== "") parts.push(seg);
  }
  return parts.join("/");
}

/**
 * The pages are written as plain Markdown so they read well on GitHub and to an agent.
 * docs-viewer has its own dialect, so the differences are bridged here instead of in the
 * files:
 *
 * - `[text](other.md)` becomes a docs-viewer cross-link to that page.
 * - Relative image paths are pointed at the served copy of docs/.
 * - A table whose cells are images becomes a run of figures (the viewer drops images in cells).
 * - `### Heading` becomes a bold line (the viewer only knows `##`).
 */
function adapt(page: Page): string {
  let text = page.content;

  text = text.replace(/((?:^\|.*\n)+)/gm, (table) => {
    if (!table.includes("![")) return table;
    const images = table.match(/!\[[^\]]*\]\([^)]+\)/g) ?? [];
    return images.join("\n\n") + "\n";
  });

  text = text.replace(/(!?)\[([^\]\n]*)\]\(([^)\s]+)\)/g, (whole, bang, label, target: string) => {
    if (/^(https?:|mailto:|#|\/)/.test(target)) return whole;
    const [file, anchor] = target.split("#");
    const resolved = resolve(page.path, file);
    if (bang) return `![${label}](/${resolved})`;
    const hit = pages.find((p) => p.path === resolved || p.path === `${resolved}/README.md`);
    if (!hit?.category || !hit.id) return whole;
    return `[[link:${hit.category}/${hit.id}${anchor ? `/${anchor}` : ""}|${label}]]`;
  });

  // Inside a table row a bare `|` would end the cell, so the cross-link's separator is escaped.
  text = text
    .split("\n")
    .map((line) => (line.startsWith("|") ? line.replace(/(\[\[link:[^\]|]+)\|/g, "$1\\|") : line))
    .join("\n");

  text = text.replace(/^#{3,}\s+(.+)$/gm, "**$1**");
  return text;
}

const categories = parseMarkdownDocs(
  pages.filter((p) => p.category).map((p) => ({ path: p.path, content: adapt(p) })),
);

export const docsConfig: DocsConfig = {
  title: "Switchboard",
  subtitle: "What it does and how to work on it",
  sections: organizeSidebar(categories, [
    { title: "", categoryIds: ["overview"] },
    { title: "Features", categoryIds: ["features"] },
    { title: "Guides", categoryIds: ["guides"] },
    { title: "Working on the code", categoryIds: ["contributing"] },
  ]),
};
