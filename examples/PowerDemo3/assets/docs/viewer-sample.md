# The Viewer, seen from a form

This document exists to be *looked at*. Everything below is here because the
Viewer draws it, so opening this file exercises the whole Markdown path in one
go: headings, prose, emphasis, lists, tables, fenced code, quotes and rules.

Switch **Layout** between `Raw`, `Web`, `Print` and `Page` while this is open.
`Raw` shows you the Markdown source exactly as it is stored. The other three
render it. That difference is the point: the stored form never changes, and what
you see is computed from it.

## What the control promises

A document is **stored in its own native form**, always. This file stays
Markdown text in memory, on disk and while it is on screen. The headings and
the table you are reading are computed from that text when the page is painted,
and thrown away again. Nothing here is converted on the way in.

That is what makes **Save As** honest. It writes the bytes this file already
had, not a re-rendering of what you are looking at.

## Formats it reads

| Format | What arrives | What does not |
|---|---|---:|
| Plain text | Any size, paged | nothing |
| Markdown | Headings, tables, lists, code | nothing |
| Images | PNG, JPEG, GIF, WebP, BMP, TIFF, SVG | video |
| PDF | Text, page geometry, search | faithful raster of complex pages |
| HTML | A subset: blocks, tables, type | scripts, grid, animation |

The last column matters more than the middle one. A viewer that quietly does
less than it claims is worse than one that says where it stops.

## Things worth trying

- [x] Open this file and switch between the four layouts
- [x] Turn on **split view** and put a second document beside it
- [ ] Search for a word and step through the matches
- [ ] Switch to the **card grid** and drag the size slider
- [ ] Start the stream and watch a conversation grow

### Emphasis, links and the rest

Inline code looks like `VWR-1::Find()`. Emphasis comes in *italic* and **bold**.
Something withdrawn reads as ~~struck through~~. A [link](https://example.test)
carries its destination.

> A quotation sits indented behind a rule of its own.
> It can run to several lines, and it keeps its shape when the text reflows.

---

## Driving it from COBOL

Every visual state is a property, and every action is a method. Nothing in the
control is reachable only by mouse.

```cobol
           MOVE "Web" TO VWR-1::Layout.
           MOVE "LeftRight" TO VWR-1::SplitMode.
           MOVE "report" TO VWR-1::View1SearchText.
           VWR-1::Find().
           VWR-1::SaveAs().
```

The same is true of the streamed conversation mode, which appends content as it
arrives rather than reloading a document:

```cobol
           MOVE "Streamed" TO VWR-1::Layout.
           VWR-1::AppendMarkdown("## A message\n\nIts body.\n\n").
           VWR-1::JumpToLatest().
```

## A note on long documents

A file far larger than memory still opens, and jumping to its end is prompt,
because the control indexes the document and keeps only a bounded window of
decoded pages. Try `viewer-sample.txt` beside this one: it carries explicit page
breaks, so **Previous page** and **Next page** move between real pages rather
than scrolling a single sheet.
