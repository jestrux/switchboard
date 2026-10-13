---
id: links-and-ports
number: 7
title: Links and ports
category: features
description: Opening links and previewing images printed in a terminal, and seeing the port a server is listening on.
---

# Links and ports

## ⌘-click links

- **What:** Links in terminal output open in the browser. Plain URLs, Markdown links
  written as `[text](url)`, and terminal hyperlinks (OSC 8) all work.
- **Reach it:** Hold ⌘ and hover to see the link underlined, then click.
- **State:** Every URL the app opens is logged to `~/Library/Logs/Switchboard/links.log`.
- **Limits:** None known.

## Image previews

- **What:** An image named in terminal output can be previewed and opened. Two kinds are
  recognised: a path to an image file, and the `[Image #N]` marker Claude Code prints for
  an image pasted into its prompt. Paths may be absolute, start with `~`, or be relative to
  the command's folder, and may contain spaces or wrap onto a second line.
- **Reach it:** Hover the path or marker, with no key held, to underline it and show a
  preview card with the file name and pixel size. Click to open it in the Mac's default
  image app, which is Preview unless you changed it. Dragging across a path still selects
  text.
- **State:** Nothing is stored for file paths. A pasted image is copied out of the Claude
  session's transcript into `~/Library/Caches/dev.waky.switchboard/pasted-images/`. Every
  image opened is logged to `~/Library/Logs/Switchboard/links.log`.
- **Limits:** A path counts only when the file exists, so a missing file or a remote image
  URL stays plain text. An `[Image #N]` marker works once its message has been sent, not
  while it is still in the prompt, and only for a Claude session that saves a transcript.
  The preview stays where it first appeared while the pointer moves along the same
  reference.

## Listening port

- **What:** When a running command prints the address it is listening on, its port shows on
  its sidebar row and in its pane bar, such as `:3000`. It is read from the output, so it is
  the port the server really took, including a fallback port.
- **Reach it:** Click the port to open `http://localhost:<port>` in the browser.
- **State:** Held by the engine while the command is alive, and cleared when it stops.
- **Limits:** A server that never prints its address shows no port. Lines about a refused
  or failed connection are ignored.
