# Announcements

Put the current announcement here as **`announcement.html`**. Every installed PowerRustCOBOL
IDE reads that one file from this folder on `main`, at start-up and every six hours, and shows
it once in a window titled **What's going on**, at 70 % of the screen.

- **Once per version of the page.** When the developer closes the window, the IDE records the
  page's identity (a hash of its bytes), and does not show that page again. Any edit to the
  file — a word, a space — makes it a new announcement, shown once more to everyone.
- **No file, no window.** Delete `announcement.html` (or leave this folder without one) and
  nothing is shown. A network that is not there, or a blank file, shows nothing either.
- **A few minutes to arrive.** GitHub caches raw files for a few minutes, so a page just
  pushed may take that long to reach an IDE that checks.

## What the page can contain

The page is drawn by the **Viewer control's HTML engine** — the same one a PowerRustCOBOL
application uses — on a white page, black text by default. It is not a browser: no script
runs and nothing is fetched.

- **Works:** headings, paragraphs, lists, tables, `<strong>`/`<em>`/`<code>`, colours, borders,
  backgrounds, padding and margins, `text-align`, and layout with `display: flex` and
  `display: grid` — styled by the page's own `<style>` block and `style` attributes.
- **Not yet:** a link is drawn as a link but does not open when clicked, and an image shows
  its `alt` text instead of the picture. Write the address out where a reader needs it.

This README is never shown; only `announcement.html` is.
