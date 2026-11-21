// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Viewer's own rules** (spec 058), for the interpreter and the desktop runner alike
//! (DESKTOP-WASM-001 T50).
//!
//! What a Viewer does when the program loads a document into it, appends to its conversation, finds
//! in it, saves, prints or shares it, and what it does with the answers the window sends back. The
//! rules act through a [`ViewerSide`]: the interpreter's object registry for an interpreted form, the
//! ABI host's control tree for a program built on the WebAssembly path. What a Viewer keeps that is
//! not a property (the bytes `LoadBytes` gave it, its conversation and its history) is a
//! [`ViewerState`], one per control.

use cobolt_forms::viewer::{AppendMode, Conversation, ConversationEntry, ConversationHistory, MessageRole};

/// What the operating system is asked to do with a Viewer's document, and later answers about.
///
/// The three are the only events a Viewer cannot raise from a flag of its own: only the OS dialog
/// knows whether the user went through with it, which is why their events arrive through
/// [`report_os_outcome`] instead of being raised from a flag here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewerOsAction {
    Print,
    Share,
    Save,
}

/// The toolbar's Save As button (`_SaveAsAsk`), the destination the program asks the window for
/// (`_SaveAsRequest`, the suggested name) and the one the window answers (`_SaveAsAnswer`, empty when
/// the operator cancelled).
pub const SAVE_AS_ASK: &str = "_SaveAsAsk";
pub const SAVE_AS_REQUEST: &str = "_SaveAsRequest";
pub const SAVE_AS_ANSWER: &str = "_SaveAsAnswer";
/// Whether the destination asked for is for `SaveAsPdf` (`1`) rather than `SaveAs`.
pub const SAVE_AS_PDF: &str = "_SaveAsPdf";
/// The toolbar's Print and Share buttons, the requests the window hands to the operating system (the
/// document's path) and its answers (`1` done, empty cancelled).
pub const PRINT_ASK: &str = "_PrintAsk";
pub const PRINT_REQUEST: &str = "_PrintRequest";
pub const PRINT_ANSWER: &str = "_PrintAnswer";
pub const SHARE_ASK: &str = "_ShareAsk";
pub const SHARE_REQUEST: &str = "_ShareRequest";
pub const SHARE_ANSWER: &str = "_ShareAnswer";

/// How often a load reports progress, in percent (R6).
///
/// The indexer itself reports every whole percent; forwarding all 101 of them would put a hundred
/// events on the dispatch queue for one `MOVE`, and a COBOL handler cannot use that resolution for
/// anything. Every tenth percent, plus a guaranteed final 100, is a progress bar a developer can
/// actually drive.
pub const PROGRESS_STEP: i64 = 10;

/// What a Viewer keeps that is not a property.
#[derive(Debug, Default)]
pub struct ViewerState {
    /// The document `LoadBytes` gave it (or a report written to it), byte for byte.
    pub bytes: Option<Vec<u8>>,
    /// Its conversation (§8), made on first use.
    pub conversation: Option<Conversation>,
    /// §8.8's history.
    pub history: ConversationHistory,
}

/// The Viewer as the rules see it: its properties and its events.
pub trait ViewerSide {
    /// The control's name.
    fn name(&self) -> &str;
    /// A property, as the program reads it (empty when it has none).
    fn get(&self, prop: &str) -> String;
    /// A property's stored value, or `None` when the control has no such property.
    fn get_opt(&self, prop: &str) -> Option<String>;
    /// Write a property as the program's own write would, the window told.
    fn set(&mut self, prop: &str, value: String);
    /// Store a property's value and nothing else: no load, and the window is not told. A mirror
    /// finishing its own write, or R4's restore of a source that failed.
    fn put_quiet(&mut self, prop: &str, value: String);
    /// Raise one of the control's events.
    fn event(&mut self, name: &str);
}

fn truthy(s: &str) -> bool {
    let t = s.trim();
    t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes") || t.eq_ignore_ascii_case("on")
}

fn parse_i(s: String) -> i64 {
    s.trim().parse::<i64>().unwrap_or(0)
}

/// §8.1's blanket override, from the value of `RenderAsHtml`: on when the control has none.
pub fn render_as_html(value: Option<&str>) -> bool {
    value
        .map(|t| {
            let t = t.trim();
            !(t.is_empty() || t == "0" || t.eq_ignore_ascii_case("false"))
        })
        .unwrap_or(true)
}

/// The control's conversation, created on first use.
///
/// §8.1's blanket override is a PROPERTY, so it can change between appends; the conversation reads it
/// each time rather than remembering whatever it was when the first chunk arrived.
pub fn conversation<'a>(state: &'a mut ViewerState, side: &dyn ViewerSide) -> &'a mut Conversation {
    let render = render_as_html(side.get_opt("RenderAsHtml").as_deref());
    let conv = state.conversation.get_or_insert_with(Conversation::new);
    conv.set_render_as_html(render);
    conv
}

/// One of the Viewer's methods (`m`, upper case), with its arguments as the program wrote them: what it
/// answers (`Some(None)` when it answers nothing), or `None` when `m` is not one of them.
pub fn method(state: &mut ViewerState, side: &mut dyn ViewerSide, m: &str, args: &[String]) -> Option<Option<String>> {
    let arg = |i: usize| args.get(i).map(|v| v.trim().to_string()).unwrap_or_default();
    let content = || args.first().cloned().unwrap_or_default();
    let answer = match m {
        "LOADBYTES" => {
            // R1's byte-buffer load. The bytes live beside the object rather than in a property: a
            // property is a `String`, and a PDF put through one would not come back byte for byte,
            // which R18's Save As depends on.
            // NOT trimmed: for every other method that is right (a COBOL `PIC X(80)` holding a path
            // arrives padded with spaces), but here the argument IS the payload, so a trim would
            // silently drop a document's leading indent or its final newline.
            //
            // A COBOL item is still fixed-length: `PIC X(100)` holding 42 characters delivers 100,
            // padded. That is the developer's to size, not something this method can guess at.
            let bytes = content().into_bytes();
            let format = cobolt_forms::viewer::detect_format(None, &bytes);
            state.bytes = Some(bytes);
            match format {
                Some(f) => {
                    side.set("Format", f.as_str().to_string());
                    side.set("Progress", "100".into());
                    side.set("LastError", String::new());
                    side.event("onLoaded");
                }
                None => {
                    side.set("LastError", cobolt_forms::viewer::ViewerLoadError::UnsupportedFormat.to_message());
                    side.event("onError");
                }
            }
            None
        }
        // R18: a path GIVEN is always the path written; R18.1's proposed default filename is the
        // interactive dialog's convenience, never this method's contract (plan.md §3).
        //
        // `SaveAs()` with NO path is the dialog's own case: it asks the host for a destination, the
        // same way `Show()` reaches a Snackbar. A pseudo-property crosses the state channel and the
        // host acts on it. The suggested name is computed HERE because only the program holds the
        // document.
        "SAVEAS" => {
            let path = arg(0);
            if path.trim().is_empty() {
                side.set(SAVE_AS_PDF, String::new());
                let suggested = suggested_filename(state, side);
                side.set(SAVE_AS_REQUEST, suggested);
            } else {
                match save_as(state, side, &path) {
                    Ok(()) => report_os_outcome(side, ViewerOsAction::Save, true),
                    Err(e) => {
                        side.set("LastError", e);
                        side.event("onError");
                    }
                }
            }
            None
        }
        // The document (or, in `Streamed`, the conversation) laid out as a PDF with its formatting
        // (operator, 2026-09-27). With no path it asks the operator, through the very Save As panel
        // `SaveAs()` uses; a flag says which of the two the answer is for.
        "SAVEASPDF" | "SAVE-AS-PDF" => {
            let path = arg(0);
            if path.trim().is_empty() {
                side.set(SAVE_AS_PDF, "1".to_string());
                let suggested = suggested_pdf_name(state, side);
                side.set(SAVE_AS_REQUEST, suggested);
            } else {
                match save_as_pdf(state, side, &path) {
                    Ok(()) => report_os_outcome(side, ViewerOsAction::Save, true),
                    Err(e) => {
                        side.set("LastError", e);
                        side.event("onError");
                    }
                }
            }
            None
        }
        // R19/R20: the OS does the printing and the sharing. The request is left for the host to pick
        // up; the Complete/Cancelled event comes back through `report_os_outcome`, because only the OS
        // dialog knows which of the two happened.
        "PRINT" => {
            ask_os(state, side, ViewerOsAction::Print);
            None
        }
        "SHARE" => {
            ask_os(state, side, ViewerOsAction::Share);
            None
        }
        // ── §8.2's append surface ──
        //
        // The mode is SPECIFIED per call and never inferred from the content (§8.5): a chunk sent as
        // Raw stays raw even when it is valid markup. The optional second argument says who the
        // message is from ("user" or "agent") and draws it in that one's bubble. Each RETURNS the new
        // message's id, so the caller can extend it with `AppendToMessage` as a streamed reply arrives.
        "APPENDHTML" | "APPEND-HTML" | "APPENDMARKDOWN" | "APPEND-MARKDOWN" | "APPENDRAW" | "APPEND-RAW" => {
            let mode = if m.contains("HTML") {
                AppendMode::Html
            } else if m.contains("MARKDOWN") {
                AppendMode::Markdown
            } else {
                AppendMode::Raw
            };
            let role = MessageRole::from_str(&arg(1));
            let new_id = conversation(state, side).append_as(mode, &content(), role);
            publish_conversation(state, side, true);
            Some(new_id)
        }
        // `AppendToMessage(messageId, content, mode)`: mode last, as §8.2 spells it. A message's
        // content REPLACED, keeping its id, role and place: a status bubble that follows the work and
        // then becomes the answer (operator, 2026-09-27). And a message taken out.
        "APPENDTOMESSAGE" | "APPEND-TO-MESSAGE" | "REPLACEMESSAGE" | "REPLACE-MESSAGE" | "REMOVEMESSAGE" | "REMOVE-MESSAGE" => {
            let id = arg(0);
            let text = args.get(1).cloned().unwrap_or_default();
            let mode = AppendMode::from_str(&arg(2));
            let conv = conversation(state, side);
            let done = if m.starts_with("APPEND") {
                conv.append_to_message(&id, mode, &text)
            } else if m.starts_with("REPLACE") {
                conv.replace_message(&id, mode, &text)
            } else {
                conv.remove_message(&id)
            };
            if done {
                publish_conversation(state, side, true);
            } else {
                // A streamed chunk for a message that never started is a caller error worth
                // reporting, not something to silently mint a new message for.
                side.set("LastError", format!("no conversation message with id {id:?}"));
                side.event("onError");
            }
            None
        }
        // ── §8.8's conversation management ──
        //
        // The ORDER matters and is tested: the pane is cleared FIRST, then the event is raised, so a
        // handler bound to either event sees an empty pane and never stale content.
        "NEWCONVERSATION" | "NEW-CONVERSATION" => {
            let was_empty = conversation(state, side).is_empty();
            archive_current(state, side);
            // AC26: an empty pane gets no history entry, and no event.
            if !was_empty {
                conversation(state, side).clear();
                side.set("ConversationId", String::new());
                publish_conversation(state, side, false);
                side.event("onConversationCreated");
            }
            None
        }
        "SELECTCONVERSATION" | "SELECT-CONVERSATION" => {
            let id = arg(0);
            archive_current(state, side);
            // The selected entry leaves history and becomes current.
            state.history.take(&id);
            publish_history(state, side);
            conversation(state, side).clear();
            side.set("ConversationId", id.clone());
            publish_conversation(state, side, false);
            // §8.8: this is the signal telling the host to start calling the append methods. The
            // control never restores content from a cache of its own; there is no cache, by design.
            side.set("ConversationId", id);
            side.event("onConversationSelected");
            None
        }
        "REGISTERCONVERSATION" | "REGISTER-CONVERSATION" => {
            let id = arg(0);
            let title = arg(1);
            state.history.push(ConversationEntry { id, title: if title.trim().is_empty() { "Conversation".into() } else { title } });
            publish_history(state, side);
            None
        }
        // §8.4's "Jump to latest", COBOL-callable. The VIEWPORT is the engine's (it is the only side
        // that knows how tall the content is), so this leaves a request the way Print and Share do.
        "JUMPTOLATEST" | "JUMP-TO-LATEST" => {
            let n = parse_i(side.get("_JumpToLatest")) + 1;
            side.set("_JumpToLatest", n.to_string());
            None
        }
        "FINDNEXT" | "FIND-NEXT" => {
            step_find(side, true);
            None
        }
        "FINDPREVIOUS" | "FIND-PREVIOUS" => {
            step_find(side, false);
            None
        }
        // R26/R31: opening and closing Find from COBOL, with the same events the toolbar button
        // raises. A boolean reads back "true"/"false" once stored, so the test is `truthy`, never "1".
        "FIND" => {
            if !args.is_empty() {
                side.set("View1SearchText", arg(0));
                side.set("View1SearchCurrentMatch", "0".into());
            }
            if !truthy(&view_prop(side, "FindOpen")) {
                side.set("View1FindOpen", "1".into());
                side.event("onFindOpened");
            }
            None
        }
        "FINDCLOSE" | "FIND-CLOSE" => {
            if truthy(&view_prop(side, "FindOpen")) {
                side.set("View1FindOpen", "0".into());
                side.event("onFindClosed");
            }
            None
        }
        _ => return None,
    };
    Some(answer)
}

/// Publish what the engine paints, and raise §8.2 item 6's `onContentRendered` unless `rendered` is
/// false: clearing a pane is not content arriving.
///
/// `onContentRendered` fires **after the layout pass**, not after the data was accepted, which here is
/// literal: `Conversation::append` computes the new chunk's layout before returning.
fn publish_conversation(state: &mut ViewerState, side: &mut dyn ViewerSide, rendered: bool) {
    let (html, empty) = {
        let conv = conversation(state, side);
        (conv.to_html(), conv.is_empty())
    };
    side.set("_ConversationHtml", html);
    side.set("_ConversationEmpty", if empty { "1" } else { "0" }.to_string());
    if rendered {
        side.event("onContentRendered");
    }
}

/// §8.8: archive whatever is open, before the pane is cleared.
///
/// **A no-op on an already-empty pane** (AC26): archiving nothing would put a spurious entry in a list
/// the developer's own UI enumerates. The title is the conversation's own first line, trimmed.
fn archive_current(state: &mut ViewerState, side: &mut dyn ViewerSide) {
    let (has_content, title, id) = {
        let conv = conversation(state, side);
        let text = conv.text();
        let first = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").to_string();
        let title: String = first.chars().take(60).collect();
        (!conv.is_empty(), if title.is_empty() { "Conversation".to_string() } else { title }, conv.current_message_id().unwrap_or("").to_string())
    };
    if !has_content {
        return;
    }
    let current = side.get("ConversationId");
    let entry_id = if current.trim().is_empty() {
        if id.is_empty() {
            "conversation".to_string()
        } else {
            id
        }
    } else {
        current
    };
    state.history.push(ConversationEntry { id: entry_id, title });
    publish_history(state, side);
}

/// §8.8's `HistoryList`, republished whenever it changes, so a COBOL program reads it as an ordinary
/// property.
fn publish_history(state: &ViewerState, side: &mut dyn ViewerSide) {
    side.set("HistoryList", state.history.to_list());
}

/// Spec 058 plan §3/§4: the unprefixed property names are **aliases** onto `View1*`, the single
/// canonical store for one view's state. A program may write either spelling; both then read back the
/// same. Called after the program wrote `prop` (`val`).
pub fn mirror_alias(side: &mut dyn ViewerSide, prop: &str, val: &str) {
    // A compiled `MOVE … TO VWR-1::View1Source` arrives as `VIEW1SOURCE`: the prefix is matched as a
    // word, not as one spelling of it.
    let prop = &match prop.get(..5) {
        Some(p) if p.eq_ignore_ascii_case("View1") => format!("View1{}", &prop[5..]),
        _ => prop.to_string(),
    };
    const ALIASED: &[&str] = &[
        "Source",
        "Page",
        "Zoom",
        "ViewMode",
        "CardSize",
        "ShowFilmstrip",
        "ScrollPosition",
        "SearchText",
        "SearchCaseSensitive",
        "SearchHighlightEnabled",
        "SearchCurrentMatch",
        "SearchMatchCount",
        "FindOpen",
    ];
    let other = if let Some(rest) = prop.strip_prefix("View1") {
        if !ALIASED.iter().any(|a| a.eq_ignore_ascii_case(rest)) {
            return;
        }
        rest.to_string()
    } else if ALIASED.iter().any(|a| a.eq_ignore_ascii_case(prop)) {
        format!("View1{prop}")
    } else {
        return;
    };
    if side.get(&other) != val {
        side.set(&other, val.to_string());
    }
    // The `View1*` spelling is the CANONICAL store, and it is the one with a seeded type, so it is the
    // one a stored value is normalised by (a boolean written as `1` reads back `true`). The unprefixed
    // alias would otherwise keep the raw `1`, leaving the two spellings of one value disagreeing about
    // it. Stored quietly, because this is a mirror finishing its own write, not a new one.
    let canonical = if prop.starts_with("View1") { prop.to_string() } else { other.clone() };
    let alias = canonical.trim_start_matches("View1").to_string();
    let settled = side.get(&canonical);
    if side.get(&alias) != settled {
        side.put_quiet(&alias, settled);
    }
}

/// One of a Viewer's view properties under **either** spelling: the canonical `View1*` first, the
/// alias when that is empty (a designed `View1SearchMatchCount` never went through [`mirror_alias`]).
fn view_prop(side: &dyn ViewerSide, name: &str) -> String {
    let prefixed = side.get(&format!("View1{name}"));
    if prefixed.is_empty() {
        side.get(name)
    } else {
        prefixed
    }
}

/// Spec 058 R28/R31: move Find to the next or previous match, wrapping.
///
/// The **match count is the view's**: the engine is what knows which text is on screen, and it
/// publishes the total as `SearchMatchCount`.
fn step_find(side: &mut dyn ViewerSide, forward: bool) {
    let as_index = |v: String| v.trim().parse::<i64>().unwrap_or(0).max(0) as usize;
    let total = as_index(view_prop(side, "SearchMatchCount"));
    // The property counts matches from 1 (0 = none); `step_match` from 0.
    let current = as_index(view_prop(side, "SearchCurrentMatch")).saturating_sub(1);
    if let Some(next) = cobolt_forms::viewer::step_match(current, total, forward) {
        side.set("View1SearchCurrentMatch", (next + 1).to_string());
    }
}

/// Open the document `source` names, reporting it the way R3/R4/R6 say: the program wrote it to
/// `Source` (or `View1Source`).
///
/// On success `Format` carries R3's resolved format, `Progress` reaches 100 and `onLoaded` is raised.
/// On failure `LastError` carries R4's message, `onError` is raised, and **the previously loaded
/// document is left displayed**: both spellings of `Source` go back to whatever last loaded.
/// Returns whether the document opened: `false` means R4's restore has run and the caller must leave
/// both spellings of `Source` alone.
pub fn open_source(side: &mut dyn ViewerSide, source: &str) -> bool {
    use cobolt_forms::viewer::{open_document, DocumentSource};

    let path = source.trim().to_string();
    if path.is_empty() {
        // Clearing the source is not a failed load.
        side.set("Format", String::new());
        side.set("Progress", "0".into());
        return true;
    }
    let resolved = cobolt_forms::assets::resolve(&path);
    let src = DocumentSource::Path(resolved.to_string_lossy().into_owned());

    let mut ticks: Vec<i64> = Vec::new();
    let mut last_step = -1i64;
    let result = open_document(&src, |pct| {
        let step = pct / PROGRESS_STEP;
        if step != last_step {
            last_step = step;
            ticks.push(pct);
        }
    });
    for pct in ticks {
        side.set("Progress", pct.to_string());
        side.event("onLoadProgress");
    }
    match result {
        Ok(index) => {
            side.set("Format", index.format.as_str().to_string());
            side.set("Progress", "100".into());
            side.set("LastError", String::new());
            side.set("_LoadedSource", path);
            side.set("_PageCount", index.page_count().to_string());
            side.event("onLoaded");
            true
        }
        Err(e) => {
            side.set("LastError", e.to_message());
            // R4: the failed name must not replace the document on screen. Stored quietly, or the
            // write would start a load all over again. BOTH spellings go back: a program may have
            // written either.
            let previous = side.get("_LoadedSource");
            for name in ["Source", "View1Source"] {
                side.put_quiet(name, previous.clone());
            }
            side.event("onError");
            false
        }
    }
}

/// Whether writing `prop` starts a load: either spelling of the source.
pub fn is_source(prop: &str) -> bool {
    prop.eq_ignore_ascii_case("Source") || prop.eq_ignore_ascii_case("View1Source")
}

/// The program wrote `prop` (`val`) on the Viewer: a source starts a load, and an aliased view
/// property is mirrored to its other spelling. The caller makes sure the writes this makes do not
/// come back here.
pub fn written(side: &mut dyn ViewerSide, prop: &str, val: &str) {
    let opened = if is_source(prop) { open_source(side, val) } else { true };
    // A FAILED load has already put both spellings back to the document that is still on screen (R4);
    // mirroring the name that failed would undo exactly that.
    if opened {
        mirror_alias(side, prop, val);
    }
}

/// The name Save As proposes, and the name a `LoadBytes` document is staged under for the OS: the
/// source's file name, else the first words of the document, else the control's name, with the
/// format's extension.
pub fn suggested_filename(state: &ViewerState, side: &dyn ViewerSide) -> String {
    let source = side.get("Source");
    let source = source.trim();
    if !source.is_empty() {
        if let Some(name) = source.rsplit(['/', '\\']).next().map(str::trim).filter(|n| !n.is_empty()) {
            return name.to_owned();
        }
    }
    let ext = cobolt_forms::viewer::extension_for(cobolt_forms::viewer::ViewerFormat::from_str(&side.get("Format")));
    let words: String = state
        .bytes
        .as_ref()
        .map(|b| String::from_utf8_lossy(&b[..b.len().min(4096)]).into_owned())
        .unwrap_or_default()
        .split_whitespace()
        .filter(|w| w.chars().any(|c| c.is_alphanumeric()))
        .take(3)
        .map(|w| w.chars().filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_').collect::<String>())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if words.is_empty() {
        format!("{}.{ext}", side.name())
    } else {
        format!("{words}.{ext}")
    }
}

/// The name the Save as PDF panel opens with: `conversation.pdf` for a conversation, else the
/// document's own proposed name as a `.pdf`.
pub fn suggested_pdf_name(state: &ViewerState, side: &dyn ViewerSide) -> String {
    if !side.get("_ConversationHtml").trim().is_empty() || side.get("Layout").trim().eq_ignore_ascii_case("Streamed") {
        return "conversation.pdf".to_string();
    }
    let name = suggested_filename(state, side);
    let stem = name.rsplit_once('.').map(|(s, _)| s).filter(|s| !s.is_empty()).unwrap_or(&name);
    format!("{stem}.pdf")
}

/// The document's ORIGINAL bytes: the file a `Source`-loaded document came from, or what `LoadBytes`
/// gave.
fn document_bytes(state: &ViewerState, source: &str) -> Result<Vec<u8>, String> {
    if !source.trim().is_empty() {
        let resolved = cobolt_forms::assets::resolve(source.trim());
        std::fs::read(&resolved).map_err(|e| format!("could not read document: {e}"))
    } else if let Some(b) = &state.bytes {
        Ok(b.clone())
    } else {
        Err("no document loaded".to_string())
    }
}

/// Spec 058 R18: write the document's ORIGINAL bytes to `path`, unmodified, and never a rendered or
/// re-encoded one. A `Source`-loaded document is copied byte for byte from the file it came from,
/// which is what makes AC6 true by construction.
pub fn save_as(state: &ViewerState, side: &dyn ViewerSide, path: &str) -> Result<(), String> {
    let dest = path.trim();
    if dest.is_empty() {
        return Err("no destination path".to_string());
    }
    let bytes = document_bytes(state, &side.get("Source"))?;
    std::fs::write(dest, &bytes).map_err(|e| format!("could not write document: {e}"))
}

/// `SaveAsPdf`: the conversation a `Streamed` Viewer holds, or its Markdown or text document, laid out
/// as a PDF at `path`; a PDF document is copied as it stands. Other formats say what the method takes.
#[cfg(feature = "pdf")]
pub fn save_as_pdf(state: &ViewerState, side: &dyn ViewerSide, path: &str) -> Result<(), String> {
    use cobolt_forms::viewer::ViewerFormat;
    let dest = std::path::Path::new(path.trim());
    if dest.as_os_str().is_empty() {
        return Err("no destination path".to_string());
    }
    let title = dest.file_stem().and_then(|s| s.to_str()).unwrap_or("Document").to_string();
    // The PDF is what the Viewer paints, so the painting follows the Viewer's own settings: its bubble
    // colours and its font size.
    let mut viewer = cobolt_forms::Control::new(side.name(), cobolt_forms::ControlType::Viewer, 0, 0);
    for key in ["UserBubbleColor", "UserBubbleTextColor", "UserBubbleBold", "AgentBubbleColor", "AgentBubbleTextColor", "FontSize"] {
        let v = side.get(key);
        if !v.trim().is_empty() {
            viewer.set_prop(key, cobolt_forms::PropValue::String(v));
        }
    }
    // A `Streamed` Viewer shows its conversation and nothing else: an empty one is "the conversation
    // is empty", not "no document".
    let html = side.get("_ConversationHtml");
    if !html.trim().is_empty() || side.get("Layout").trim().eq_ignore_ascii_case("Streamed") {
        return cobolt_forms::viewer_pdf::conversation_to_pdf(&viewer, &html, &title, dest);
    }
    let source = side.get("Source");
    let bytes = document_bytes(state, &source)?;
    match ViewerFormat::from_str(&side.get("Format")) {
        ViewerFormat::Pdf => std::fs::write(dest, &bytes).map_err(|e| format!("could not write the PDF: {e}")),
        ViewerFormat::Markdown | ViewerFormat::Text => {
            let hint = (!source.trim().is_empty()).then(|| source.trim().to_string());
            let text = cobolt_forms::viewer::markdown_text(hint.as_deref(), &bytes);
            cobolt_forms::viewer_pdf::markdown_to_pdf(&viewer, &text, &title, dest)
        }
        other => Err(format!("Save as PDF takes a conversation, a Markdown or text document, or a PDF — not {}", other.as_str())),
    }
}

#[cfg(not(feature = "pdf"))]
pub fn save_as_pdf(_state: &ViewerState, _side: &dyn ViewerSide, _path: &str) -> Result<(), String> {
    Err("this application was built without Save as PDF".to_string())
}

/// Spec 058 R32: the OS reported back on a Print / Share / Save As handoff.
pub fn report_os_outcome(side: &mut dyn ViewerSide, action: ViewerOsAction, completed: bool) {
    let event = match (action, completed) {
        (ViewerOsAction::Print, true) => "onPrintComplete",
        (ViewerOsAction::Print, false) => "onPrintCancelled",
        (ViewerOsAction::Share, true) => "onShareComplete",
        (ViewerOsAction::Share, false) => "onShareCancelled",
        (ViewerOsAction::Save, true) => "onSaveComplete",
        (ViewerOsAction::Save, false) => "onSaveCancelled",
    };
    side.event(event);
}

/// Ask the window to hand the document to the operating system: its path, for a `Source`-loaded one,
/// or the bytes `LoadBytes` gave staged in the temporary folder under the name Save As would propose.
pub fn ask_os(state: &ViewerState, side: &mut dyn ViewerSide, action: ViewerOsAction) {
    let prop = match action {
        ViewerOsAction::Print => PRINT_REQUEST,
        ViewerOsAction::Share => SHARE_REQUEST,
        ViewerOsAction::Save => SAVE_AS_REQUEST,
    };
    let source = side.get("Source");
    let source = source.trim();
    if !source.is_empty() {
        let resolved = cobolt_forms::assets::resolve(source);
        side.set(prop, resolved.to_string_lossy().into_owned());
        return;
    }
    let Some(bytes) = state.bytes.clone() else {
        side.set("LastError", "no document loaded".to_string());
        side.event("onError");
        return;
    };
    let name = suggested_filename(state, side);
    let path = std::env::temp_dir().join("powerrustcobol-viewer").join(name);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match std::fs::write(&path, &bytes) {
        Ok(()) => side.set(prop, path.to_string_lossy().into_owned()),
        Err(e) => {
            side.set("LastError", format!("could not stage the document: {e}"));
            side.event("onError");
        }
    }
}

/// The toolbar's Save As button: ask the window for a destination, with the name the document
/// proposes.
pub fn save_as_ask(state: &ViewerState, side: &mut dyn ViewerSide) {
    side.set(SAVE_AS_PDF, String::new());
    let suggested = suggested_filename(state, side);
    side.set(SAVE_AS_REQUEST, suggested);
}

/// The window answered a Print or Share hand-off: `1` done, anything else cancelled.
pub fn os_answer(side: &mut dyn ViewerSide, action: ViewerOsAction, answer: &str) {
    let (request, answer_prop) = match action {
        ViewerOsAction::Print => (PRINT_REQUEST, PRINT_ANSWER),
        ViewerOsAction::Share => (SHARE_REQUEST, SHARE_ANSWER),
        ViewerOsAction::Save => (SAVE_AS_REQUEST, SAVE_AS_ANSWER),
    };
    side.set(request, String::new());
    side.set(answer_prop, String::new());
    report_os_outcome(side, action, answer.trim() == "1");
}

/// The window answered a Save As: the destination chosen, or empty when the operator cancelled. The
/// document (or, after `SaveAsPdf`, its PDF) is written there.
pub fn save_as_answer(state: &ViewerState, side: &mut dyn ViewerSide, path: &str) {
    side.set(SAVE_AS_REQUEST, String::new());
    side.set(SAVE_AS_ANSWER, String::new());
    let as_pdf = side.get(SAVE_AS_PDF).trim() == "1";
    side.set(SAVE_AS_PDF, String::new());
    if path.trim().is_empty() {
        report_os_outcome(side, ViewerOsAction::Save, false);
        return;
    }
    let written = if as_pdf { save_as_pdf(state, side, path) } else { save_as(state, side, path) };
    match written {
        Ok(()) => report_os_outcome(side, ViewerOsAction::Save, true),
        Err(e) => {
            side.set("LastError", e);
            side.event("onError");
        }
    }
}

/// Where the window's write `prop` falls among the inputs one turn of the program's input loop carries
/// out: the Save As asks first, then the Save As answers, the Print and Share asks, and their answers,
/// each kind in the order the window sent them. `None` for any other write.
pub fn input_rank(prop: &str) -> Option<u8> {
    match prop {
        SAVE_AS_ASK => Some(0),
        SAVE_AS_ANSWER => Some(1),
        PRINT_ASK | SHARE_ASK => Some(2),
        PRINT_ANSWER | SHARE_ANSWER => Some(3),
        _ => None,
    }
}

/// What the window sent for a Viewer's toolbar or a hand-off (`prop`, `value`), as the program's input
/// loop takes it in: `true` when it was one of the Viewer's and was carried out. The value itself is
/// stored by the caller, as any input is, before this runs.
pub fn input(state: &ViewerState, side: &mut dyn ViewerSide, prop: &str, value: &str) -> bool {
    let asked = !matches!(value.trim(), "" | "0");
    match prop {
        SAVE_AS_ASK if asked => save_as_ask(state, side),
        SAVE_AS_ANSWER => save_as_answer(state, side, value),
        PRINT_ASK if asked => ask_os(state, side, ViewerOsAction::Print),
        SHARE_ASK if asked => ask_os(state, side, ViewerOsAction::Share),
        PRINT_ANSWER => os_answer(side, ViewerOsAction::Print, value),
        SHARE_ANSWER => os_answer(side, ViewerOsAction::Share, value),
        _ => return false,
    }
    true
}
