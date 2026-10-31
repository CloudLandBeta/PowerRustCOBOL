// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The closed vocabulary of control and collection **methods**.
//!
//! Shared by the runtime, which decides with it whether `X::name(…)` is a call
//! or a collection subscript (`Items(4)`), and by the parser, which ends a
//! MOVE's receiving fields at a method call — a method never receives a value,
//! so `ME::SetProperty(…)` on the line after `MOVE A TO B` is the next
//! statement, not a second receiver of the MOVE.

/// The methods of an ordinary DATA ITEM's value — `WS-TEXT::Trim()`,
/// `WS-CSV::Split(",")(2)` — in EVERY spelling the evaluator accepts. A data
/// item has no others: the runtime and the analyser both ask this list, so a
/// name that is not on it is an error rather than an expression that quietly
/// comes out empty.
///
/// This is the list of what RUNS. What a developer is told to WRITE is
/// [`CANONICAL_DATA_ITEM_METHODS`]; the other spellings stay on this list so
/// that no program written before the single spelling was chosen breaks, and
/// [`canonical_data_item_method`] maps each of them to the one to write.
pub const DATA_ITEM_METHODS: &[&str] = &[
    "Trim",
    "UpperCase",
    "ToUpperCase",
    "Upper",
    "LowerCase",
    "ToLowerCase",
    "Lower",
    "Replace",
    "Len",
    "Length",
    "Split",
];

/// The ONE spelling of each value method of a data item — the one the
/// documentation teaches, IntelliSense offers and the editor rewrites the
/// others to (operator, 2026-10-10: "remove the redundancy of method naming").
pub const CANONICAL_DATA_ITEM_METHODS: &[&str] = &["Trim", "Upper", "Lower", "Replace", "Len", "Split"];

/// The spellings that still run but are no longer written, each with the
/// canonical spelling it stands for. Matched without regard to case.
pub const DATA_ITEM_METHOD_ALIASES: &[(&str, &str)] = &[
    ("UpperCase", "Upper"),
    ("ToUpperCase", "Upper"),
    ("LowerCase", "Lower"),
    ("ToLowerCase", "Lower"),
    ("Length", "Len"),
];

/// `true` if `name` is a value method a data item answers (see
/// [`DATA_ITEM_METHODS`]); case does not matter.
pub fn is_data_item_method(name: &str) -> bool {
    DATA_ITEM_METHODS.iter().any(|m| m.eq_ignore_ascii_case(name.trim()))
}

/// The canonical spelling `name` should be written as, when `name` is an older
/// spelling of a value method: `Some("Upper")` for `UpperCase`, `ToUpperCase`
/// (and `touppercase`, `UPPERCASE` …), `Some("Lower")` for `LowerCase` and
/// `ToLowerCase`, `Some("Len")` for `Length`.
///
/// `None` for a name that is already canonical — `Upper`, `Trim`, `len` — and
/// for every name that is not a value method at all. Case is never "corrected"
/// on a canonical name: COBOL does not distinguish it, and rewriting `upper`
/// to `Upper` would be churn with no meaning.
pub fn canonical_data_item_method(name: &str) -> Option<&'static str> {
    let name = name.trim();
    DATA_ITEM_METHOD_ALIASES
        .iter()
        .find(|(alias, _)| alias.eq_ignore_ascii_case(name))
        .map(|(_, canonical)| *canonical)
}

/// What a data item does answer, as the sentence an error ends with. The older
/// spellings are named so that a developer who meets one in somebody else's
/// code learns that it works, and which spelling is the current one.
pub const DATA_ITEM_METHODS_TEXT: &str = "Trim, Upper, Lower, Replace, Len and Split \
(UpperCase, ToUpperCase, LowerCase, ToLowerCase and Length are older spellings that still work)";

/// `true` if `name` is a recognised control/collection method. A `GET-`/`SET-`
/// prefix is always a method (explicit accessor, spec 010).
pub fn is_known_method(name: &str) -> bool {
    let m = name.to_ascii_uppercase();
    if m.starts_with("GET-") || m.starts_with("SET-") {
        return true;
    }
    matches!(
        m.as_str(),
        // Universal lifecycle / visibility / geometry
        "SHOW" | "HIDE" | "ENABLE" | "DISABLE" | "SETFOCUS" | "FOCUS"
            | "BRINGTOFRONT" | "SENDTOBACK" | "REFRESH" | "VALIDATE"
            | "MOVETO" | "RESIZE"
        // Generic / text / caption
            | "SETPROPERTY" | "GETPROPERTY" | "SETCAPTION" | "SETTEXT"
            | "GETCAPTION" | "GETTEXT" | "APPENDTEXT" | "SETCOLOR" | "SELECTALL"
            | "CLEAR"
        // Checkbox / radio
            | "ISCHECKED" | "SETCHECKED" | "SELECT" | "TOGGLE"
        // Numeric value
            | "SETVALUE" | "GETVALUE" | "INCREMENT" | "DECREMENT" | "RESET"
        // Items / list / combo
            | "ADDITEM" | "REMOVEITEM" | "REMOVEAT" | "GETSELECTED" | "GETSELECTEDINDEX"
            | "GETINDEX" | "SETSELECTEDINDEX" | "SETINDEX" | "GETCOUNT"
            | "LOADFROMFILE"
        // DataGrid
            | "GETROWCOUNT" | "GETCELLVALUE" | "SETCELLVALUE" | "ADDROW"
            | "DELETEROW" | "CLEARROWS" | "SORT" | "SETFILTER" | "CLEARFILTERS"
            | "FREEZECOLUMNS" | "FREEZEROWS" | "SETROWHEIGHT" | "SETCOLUMNWIDTH" | "SETCOLUMNTITLE"
            | "GETSELECTEDTEXT" | "COPYSELECTION" | "EXPORTCSV"
        // TreeView — walking the tree, and reading a node. Every one takes the
        // node's INDEX, so every one MUST be listed here: an unlisted name
        // parses its parens as a collection subscript instead of as a call, and
        // `TV::NodeParent(3)` would silently mean "element 3 of NodeParent".
            | "ADDNODE"
            | "REMOVENODE" | "EXPANDALL" | "COLLAPSEALL" | "GETSELECTEDNODE" | "SETSELECTEDNODE"
            // Spec 072 — AgentObject tools.
            | "ADDTOOL" | "ADDTOOLPARAMETER" | "REMOVETOOL" | "SETTOOLRESULT" | "ALLOWFILE" | "DENYFILE"
            | "REGISTERFILE" | "UNREGISTERFILE"
            // Spec 068 — the KnowledgeBase control.
            | "CREATECOLLECTION" | "REMOVECOLLECTION" | "LISTCOLLECTIONS" | "GETCOLLECTION"
            | "LISTDOCUMENTS" | "GETDOCUMENT" | "GETRESULTDOCUMENT" | "GETRESULTHEADING"
            | "GETRESULTPASSAGE" | "GETRESULTSCORE" | "ADDDOCUMENT" | "UPDATEDOCUMENT"
            | "IMPORTDOCUMENT" | "DELETEDOCUMENT" | "REINDEX" | "FETCHMODEL"
            | "ALLOWKNOWLEDGEBASE" | "DENYKNOWLEDGEBASE"
            // (SEARCH and CANCEL are listed below, for WebSearch and the async
            // controls.)
            // Spec 066 — SideMenu rows added at run time.
            | "ADDSECTION" | "SETITEMLABEL" | "SETITEMICON" | "SETITEMBADGE" | "SETITEMENABLED"
            | "ACTIVATEITEM"
            | "SETITEMACTION" | "SETITEMCONTROL" | "HASITEM"
            | "NODECOUNT" | "NODEINDEXOF" | "NODETEXT" | "NODENAME" | "NODEPATH"
            | "NODELEVEL" | "NODEICON" | "NODECOLOR" | "NODECOLOUR"
            | "NODEBACKCOLOR" | "NODEBACKGROUND" | "NODECHILDCOUNT"
            | "NODEHASCHILDREN" | "NODEPARENT" | "NODEFIRSTCHILD"
            | "NODELASTCHILD" | "NODENEXTSIBLING" | "NODEPREVSIBLING"
            | "NODEPREVIOUSSIBLING" | "NODECHECKED" | "NODECOLLAPSED"
        // WebSearch. `Search()` was UNSPELLABLE: an unlisted name parses its
        // parens as a collection subscript, so `WEB-FIND::Search()` meant
        // "element Search of nothing" and did nothing at all — no request, no
        // event, not even an error (operator, 2026-09-08: "search does not
        // work"). Its four accessors sat in the same position.
            | "SEARCH" | "RESULTCOUNT" | "TOPTITLE" | "TOPSNIPPET" | "TOPLINK"
        // Maps — every one of its data methods, for the same reason.
            | "ADDMARKER" | "REMOVEMARKER" | "ADDROUTE" | "REMOVEROUTE"
            | "CLEARROUTES" | "ADDREGION" | "REMOVEREGION" | "CLEARREGIONS"
            | "GEOCODE" | "REVERSEGEOCODE" | "DIRECTIONS" | "DISTANCEMATRIX"
            | "PLACESSEARCH" | "TRACEROAD"
        // The async lifecycle every non-visual service control shares.
            | "CANCEL" | "ISBUSY"
        // Selection, on the controls that have one.
            | "ISSELECTED" | "SETSELECTED"
        // FileDropZone
            | "COMMITFILES"
        // Databound controls (DataGrid + repeating GroupBox/ControlArray)
            | "REFRESHBINDING"
        // DataGrid write-back (spec 094): confirm or discard the pending edits;
        // (spec 095) `Save()` writes them as one transaction.
            | "COMMITEDIT" | "DISCARDEDIT" | "SAVE"
        // Charts (AddPoint appends one label/value point; Clear/Refresh above)
            | "ADDPOINT" | "ADD-POINT"
        // Snackbar (055) — `Show()` mints a notification, `DismissAll()` clears
        // this control's, `Clear()` (above) empties the button row and
        // `AddButton()` declares one button. They must all be listed, arguments
        // or not: an unlisted name parses its parens as a collection subscript,
        // so `SNACK-1::AddButton("id=undo")` would silently mean "element … of
        // AddButton" rather than a call.
            | "SHOW" | "DISMISSALL" | "ADDBUTTON" | "ADD-BUTTON"
        // Viewer (058) — same rule as Snackbar's above: an unlisted name
        // parses its parens as a collection subscript, so `VWR-1::Print()`
        // would silently mean "element … of Print".
            | "LOADBYTES" | "SAVEAS" | "SAVEASPDF" | "SAVE-AS-PDF" | "PRINT" | "SHARE"
            | "FIND" | "FINDNEXT" | "FIND-NEXT" | "FINDPREVIOUS" | "FIND-PREVIOUS"
            | "FINDCLOSE" | "FIND-CLOSE"
        // Viewer conversation mode (058 §8)
            | "APPENDHTML" | "APPEND-HTML" | "APPENDMARKDOWN" | "APPEND-MARKDOWN"
            | "APPENDRAW" | "APPEND-RAW" | "APPENDTOMESSAGE" | "APPEND-TO-MESSAGE"
            | "REPLACEMESSAGE" | "REPLACE-MESSAGE" | "REMOVEMESSAGE" | "REMOVE-MESSAGE"
            | "JUMPTOLATEST" | "JUMP-TO-LATEST"
            | "NEWCONVERSATION" | "NEW-CONVERSATION"
            | "SELECTCONVERSATION" | "SELECT-CONVERSATION"
            | "REGISTERCONVERSATION" | "REGISTER-CONVERSATION"
        // Timer / animation
            | "START" | "STOP" | "SETINTERVAL" | "ISENABLED"
            | "PLAYANIMATION" | "PLAY" | "STOPANIMATION" | "PAUSE"
        // Panel / GroupBox (spec 090)
            | "EXPAND" | "COLLAPSE"
        // Agent / window / SQL / HTTP
            | "CLOSE" | "GETRESULT" | "SETTITLE" | "SETPROMPT" | "SETMODEL"
            | "ASK" | "GET" | "POST" | "PUT" | "DELETE" | "CALL" | "SETHEADER"
            | "CLEARHEADERS" | "SETTIMEOUT" | "OPEN" | "EXECUTE" | "EXEC"
            | "QUERY" | "FETCH" | "FETCHALL"
        // Hyperlink (WASM-006 R12): `Open` is listed above; `OpenIn(target)` takes `Self`,
        // `NewTab`, `NewWindow`, `Parent` or `Top`.
            | "SETURL" | "GETURL" | "OPENIN"
        // IndexedFile PartiQL (spec 108): `Execute`, `Fetch`, `Get`, `ColumnNames` and
        // `ColumnCount` are listed above for other controls; this one is new.
            | "EXECUTETRANSACTION"
        // SqlDatabase transactions (spec 095): the control's own verbs, so a program need
        // not send `BEGIN` as text. Listed for the usual reason — an unlisted name parses
        // its parens as a collection subscript.
            | "BEGINTRANSACTION" | "COMMIT" | "ROLLBACK" | "ISINTRANSACTION"
        // SqlDatabase result-set columns — every one MUST be listed here, or
        // `Db::ColumnName(2)` parses its parens as a collection subscript and
        // silently means "element 2 of ColumnName". Equally, a name added here
        // stops being usable as a PROPERTY on every control: `Columns` is a
        // DataGrid property, which is why the tab-joined accessor is called
        // `ColumnNames`.
            | "COLUMNNAMES" | "COLUMNCOUNT" | "COLUMNNAME"
        // AWS controls (078) — the route table's operations plus the row-set
        // accessors. `Call` is listed above.
            | "INVOKE" | "LISTFUNCTIONS" | "LISTTOOLS" | "GETROW" | "GETFIELD"
            | "LISTKNOWLEDGEBASES" | "RECORDEVENT" | "RETRIEVE" | "LISTTABLES"
            | "APPENDROWS" | "STARTJOBRUN" | "GETJOBRUN" | "STARTCRAWLER" | "GETTABLESCHEMA"
            | "ANALYZEDOCUMENT" | "CONFIRM" | "DELETEITEM" | "DELETEOBJECT" | "DESCRIBE" | "DETECTENTITIES" | "DETECTFACES" | "DETECTKEYPHRASES" | "DETECTLABELS" | "DETECTLANGUAGE" | "DETECTSENTIMENT" | "DETECTTEXT" | "GETATTRIBUTE" | "GETITEM" | "GETOBJECT" | "LIST" | "PUTITEM" | "PUTOBJECT" | "PUTVECTORS" | "QUERYVECTORS" | "SCAN" | "SIGNIN" | "SIGNOUT" | "SIGNUP" | "SYNTHESIZE" | "UPDATEITEM"
        // AWS controls (092) — RecognizeCelebrities, ListBuckets, AwsCall.
            | "RECOGNIZECELEBRITIES" | "LISTBUCKETS" | "AWSCALL"
        // Collection verbs + scalar transforms (exec_member_method)
            | "COUNT" | "SIZE" | "REMOVE" | "ADD" | "APPEND"
            | "TOUPPERCASE" | "UPPERCASE" | "UPPER"
            | "TOLOWERCASE" | "LOWERCASE" | "LOWER" | "TRIM" | "LEN" | "LENGTH"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hyperlink_methods_are_known() {
        for m in ["SetUrl", "GetUrl", "Open", "OpenIn", "SetCaption", "GetCaption"] {
            assert!(is_known_method(m), "{m} must parse as a method call");
        }
    }

    #[test]
    fn the_indexed_file_partiql_methods_are_known() {
        for m in ["Execute", "ExecuteTransaction", "Fetch", "Get", "ColumnNames", "ColumnCount"] {
            assert!(is_known_method(m), "{m} must parse as a method call");
        }
    }

    #[test]
    fn the_canonical_list_is_a_subset_of_what_runs_and_the_aliases_are_the_rest() {
        for m in CANONICAL_DATA_ITEM_METHODS {
            assert!(is_data_item_method(m), "{m} must run");
            assert_eq!(canonical_data_item_method(m), None, "{m} is already canonical");
        }
        for (alias, canonical) in DATA_ITEM_METHOD_ALIASES {
            assert!(is_data_item_method(alias), "{alias} must still run");
            assert!(CANONICAL_DATA_ITEM_METHODS.contains(canonical), "{alias} -> {canonical}");
        }
        // Every spelling that runs is canonical or an alias of a canonical one — nothing is in neither.
        for m in DATA_ITEM_METHODS {
            let canonical = CANONICAL_DATA_ITEM_METHODS.iter().any(|c| c.eq_ignore_ascii_case(m));
            let alias = canonical_data_item_method(m).is_some();
            assert!(canonical ^ alias, "{m} must be exactly one of canonical / alias");
        }
        assert_eq!(
            DATA_ITEM_METHODS.len(),
            CANONICAL_DATA_ITEM_METHODS.len() + DATA_ITEM_METHOD_ALIASES.len()
        );
    }

    #[test]
    fn each_older_spelling_maps_to_its_canonical_one_whatever_the_case() {
        for (name, want) in [
            ("UpperCase", "Upper"),
            ("ToUpperCase", "Upper"),
            ("toUpperCase", "Upper"),
            ("UPPERCASE", "Upper"),
            ("touppercase", "Upper"),
            ("LowerCase", "Lower"),
            ("ToLowerCase", "Lower"),
            ("tolowercase", "Lower"),
            ("Length", "Len"),
            ("LENGTH", "Len"),
            ("length", "Len"),
            (" Length ", "Len"),
        ] {
            assert_eq!(canonical_data_item_method(name), Some(want), "{name}");
        }
    }

    #[test]
    fn a_canonical_or_unknown_name_has_no_canonical_rewrite() {
        for name in ["Upper", "UPPER", "upper", "Lower", "Len", "len", "Trim", "Replace", "Split", "Contains", "Value", "", "Size"] {
            assert_eq!(canonical_data_item_method(name), None, "{name}");
        }
    }

    #[test]
    fn the_error_sentence_names_the_canonical_set_and_says_the_old_spellings_work() {
        assert!(DATA_ITEM_METHODS_TEXT.starts_with("Trim, Upper, Lower, Replace, Len and Split"));
        for alias in ["UpperCase", "ToUpperCase", "LowerCase", "ToLowerCase", "Length"] {
            assert!(DATA_ITEM_METHODS_TEXT.contains(alias), "{alias}");
        }
        assert!(DATA_ITEM_METHODS_TEXT.contains("still work"));
    }
}
