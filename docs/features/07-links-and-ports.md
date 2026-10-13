---
id: links-and-ports
number: 7
title: Links and ports
category: features
description: Opening links printed in a terminal, and seeing the port a server is listening on.
---

# Links and ports

## ⌘-click links

- **What:** Links in terminal output open in the browser. Plain URLs, Markdown links
  written as `[text](url)`, and terminal hyperlinks (OSC 8) all work.
- **Reach it:** Hold ⌘ and hover to see the link underlined, then click.
- **State:** Every URL the app opens is logged to `~/Library/Logs/Switchboard/links.log`.
- **Limits:** None known.

## Listening port

- **What:** When a running command prints the address it is listening on, its port shows on
  its sidebar row and in its pane bar, such as `:3000`. It is read from the output, so it is
  the port the server really took, including a fallback port.
- **Reach it:** Click the port to open `http://localhost:<port>` in the browser.
- **State:** Held by the engine while the command is alive, and cleared when it stops.
- **Limits:** A server that never prints its address shows no port. Lines about a refused
  or failed connection are ignored.
