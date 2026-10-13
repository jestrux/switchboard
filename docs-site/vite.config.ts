import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// The pages are the Markdown files in ../docs. This app only renders them.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  // Serves ../docs/media at /media, so the screenshots the pages reference resolve.
  publicDir: "../docs",
  server: { port: 5199, fs: { allow: [".."] } },
  resolve: {
    // docs-viewer brings its own React and router; two copies break the production build.
    dedupe: ["react", "react-dom", "react-router", "react-router-dom"],
  },
});
