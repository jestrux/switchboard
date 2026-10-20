import fs from "node:fs";
import type { ServerResponse } from "node:http";
import path from "node:path";
import { defineConfig, type Plugin } from "vite";
import { loadBoard, recordSend } from "./server/board";

// The board this repo keeps for itself: the board's own plan, as cards.
const BOARD_DIR = path.resolve(__dirname, process.env.BOARD_DIR ?? "../.switchboard/board");

const TYPES: Record<string, string> = { ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".png": "image/png", ".gif": "image/gif", ".webp": "image/webp", ".mp4": "video/mp4", ".mp3": "audio/mpeg", ".svg": "image/svg+xml" };

// The page reads data.js (when /events says something changed, and every 15 s besides) and
// posts its sends to /answers, as it does when Flex serves it.
function boardApi(): Plugin {
  return {
    name: "board-api",
    configureServer(server) {
      // Live arrival: tell every open page when a card, the board or the log changes. The
      // author's own sends (state/) and private media don't need a re-read.
      const pages = new Set<ServerResponse>();
      let pending: NodeJS.Timeout | undefined;
      const watcher = fs.watch(BOARD_DIR, { recursive: true }, (_event, file) => {
        const f = String(file ?? "");
        if (f.startsWith("state") || f.startsWith("private") || f.endsWith(".tmp")) return;
        clearTimeout(pending);
        pending = setTimeout(() => pages.forEach((p) => p.write("data: change\n\n")), 80);
      });
      // The plan files that steps show (a step's `doc`) are watched too, so an edited plan
      // reaches an open step panel.
      const plans = path.resolve(BOARD_DIR, "..", "..", "plans");
      const planWatcher = fs.existsSync(plans)
        ? fs.watch(plans, () => { clearTimeout(pending); pending = setTimeout(() => pages.forEach((p) => p.write("data: change\n\n")), 80); })
        : null;
      // Vite restarts the server when this file changes; don't leave the old watchers behind.
      server.httpServer?.on("close", () => { watcher.close(); planWatcher?.close(); });
      server.middlewares.use((req, res, next) => {
        const url = (req.url ?? "").split("?")[0];
        if (req.method === "GET" && url === "/events") {
          res.writeHead(200, { "Content-Type": "text/event-stream", "Cache-Control": "no-store", Connection: "keep-alive" });
          res.write(": connected\n\n");
          pages.add(res);
          req.on("close", () => pages.delete(res));
          return;
        }
        if (req.method === "GET" && url === "/data.js") {
          res.setHeader("Content-Type", "text/javascript; charset=utf-8");
          res.setHeader("Cache-Control", "no-store");
          res.end("window.BOARD = " + JSON.stringify(loadBoard(BOARD_DIR)) + ";\n");
          return;
        }
        if (req.method === "POST" && url === "/answers") {
          let body = "";
          req.on("data", (c) => (body += c));
          req.on("end", () => {
            try {
              recordSend(BOARD_DIR, JSON.parse(body));
              res.statusCode = 204;
            } catch {
              res.statusCode = 400;
            }
            res.end();
          });
          return;
        }
        if (req.method === "GET" && url.startsWith("/media/")) {
          const rel = decodeURIComponent(url.slice("/media/".length));
          const roots = [path.join(BOARD_DIR, "media"), path.join(BOARD_DIR, "private", "media")];
          const file = roots
            .map((r) => [r, path.resolve(r, rel)])
            .find(([r, f]) => f.startsWith(r + path.sep) && fs.existsSync(f) && fs.statSync(f).isFile())?.[1];
          if (!file) return next();
          res.setHeader("Content-Type", TYPES[path.extname(file).toLowerCase()] ?? "application/octet-stream");
          fs.createReadStream(file).pipe(res);
          return;
        }
        next();
      });
    },
  };
}

export default defineConfig({
  plugins: [boardApi()],
  // On the local network too, so the phone can open a card (Fuchi's NOTIFY_PLAN).
  server: { port: 5198, strictPort: true, host: true },
});
