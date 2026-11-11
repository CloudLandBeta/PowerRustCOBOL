# Announcements

Put the current announcement here, one page per language. Every installed PowerRustCOBOL IDE
reads the page of **its own language** from this folder on `main`, at start-up and every six
hours, and shows it once in a window titled **What's going on** (in the IDE's language), at
70 % of the screen.

| Language   | File                   |
| ---------- | ---------------------- |
| English    | `announcement.html`    |
| Spanish    | `announcement.es.html` |
| Portuguese | `announcement.pt.html` |
| Japanese   | `announcement.ja.html` |
| Chinese    | `announcement.zh.html` |
| French     | `announcement.fr.html` |

- **English is the page every IDE falls back to.** `announcement.html` keeps its name, so an
  IDE that has no page in its language — or an older IDE — reads it. Write it first, and write
  the five others in the same push: an IDE shows the page it finds, so a language whose page is
  not there yet reads the English one.
- **Once per version of a page.** When the developer closes the window, the IDE records the
  page's identity (a hash of its bytes), and does not show that page again. Any edit to a
  file — a word, a space — makes it a new announcement, shown once more to everyone who reads
  that file. A developer who changes the IDE's language reads the page of the new language the
  next time the IDE looks, and it is shown once as well.
- **The same page in every language.** Keep the style sheet, the structure and every `<code>`
  item (COBOL, commands, paths) identical to the English page, and translate only the words
  around them; menu and button names stay in English, as in the Developer's Guide. A test
  (`every_language_has_a_page_made_like_the_english_one`) holds the pages to that.
- **No file, no window.** To retire an announcement delete **all six** files (or leave this
  folder without them) and nothing is shown. A network that is not there, or a blank file,
  shows nothing either.
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

This README is never shown; only the announcement pages are.
