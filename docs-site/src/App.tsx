import { HashRouter, Routes, Route, Navigate } from "react-router-dom";
import { DocsProvider, DocsViewer } from "@jestrux/docs-viewer";
import "@jestrux/docs-viewer/styles";
import { docsConfig } from "./docs-config";

// Switchboard's Graphite palette.
const theme = {
  primary: "#4A62E0",
  primaryDark: "#7C9CFF",
  light: {
    background: "#FFFFFF",
    foreground: "#16181D",
    card: "#FFFFFF",
    "card-foreground": "#16181D",
    primary: "#4A62E0",
    "primary-foreground": "#FFFFFF",
    muted: "#F4F5F7",
    "muted-foreground": "#666B78",
    border: "#E3E5EA",
    sidebar: "#F7F8FA",
    "sidebar-foreground": "#16181D",
    "sidebar-border": "#E3E5EA",
    "sidebar-accent": "#ECEEF3",
    "sidebar-accent-foreground": "#16181D",
  },
  dark: {
    background: "#141519",
    foreground: "#E8EAF0",
    card: "#1A1C21",
    "card-foreground": "#E8EAF0",
    primary: "#7C9CFF",
    "primary-foreground": "#10121A",
    muted: "#1D2026",
    "muted-foreground": "#8D93A1",
    border: "#2A2D35",
    sidebar: "#1A1C21",
    "sidebar-foreground": "#E8EAF0",
    "sidebar-border": "#2A2D35",
    "sidebar-accent": "#23262E",
    "sidebar-accent-foreground": "#E8EAF0",
  },
};

// Ask AI in the search palette (⌘K). The reader pastes their own Anthropic or OpenAI key,
// which stays in their browser's localStorage, and the browser calls the provider directly.
const ai = true;

export default function App() {
  return (
    <HashRouter>
      <Routes>
        <Route path="/" element={<Navigate to="/overview/features" replace />} />
        <Route
          path="/:categoryId/:sectionId/:subsectionId?"
          element={
            <DocsProvider config={{ ...docsConfig, theme, ai }}>
              <DocsViewer />
            </DocsProvider>
          }
        />
      </Routes>
    </HashRouter>
  );
}
