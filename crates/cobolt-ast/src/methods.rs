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
            | "ADDITEM" | "REMOVEITEM" | "GETSELECTED" | "GETSELECTEDINDEX"
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
            | "SETITEMACTION" | "HASITEM"
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
        // Agent / window / SQL / HTTP
            | "CLOSE" | "GETRESULT" | "SETTITLE" | "SETPROMPT" | "SETMODEL"
            | "ASK" | "GET" | "POST" | "PUT" | "DELETE" | "CALL" | "SETHEADER"
            | "CLEARHEADERS" | "SETTIMEOUT" | "OPEN" | "EXECUTE" | "EXEC"
            | "QUERY" | "FETCH" | "FETCHALL"
        // SqlDatabase result-set columns — every one MUST be listed here, or
        // `Db::ColumnName(2)` parses its parens as a collection subscript and
        // silently means "element 2 of ColumnName". Equally, a name added here
        // stops being usable as a PROPERTY on every control: `Columns` is a
        // DataGrid property, which is why the tab-joined accessor is called
        // `ColumnNames`.
            | "COLUMNNAMES" | "COLUMNCOUNT" | "COLUMNNAME"
        // Collection verbs + scalar transforms (exec_member_method)
            | "COUNT" | "SIZE" | "REMOVE" | "ADD" | "APPEND"
            | "TOUPPERCASE" | "UPPERCASE" | "UPPER"
            | "TOLOWERCASE" | "LOWERCASE" | "LOWER" | "TRIM" | "LEN" | "LENGTH"
    )
}
