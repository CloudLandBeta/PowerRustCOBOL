// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A form's controls and the host: `Ctrl::Property`, `Ctrl::Method(args)`,
//! `INVOKE`, the built-ins that belong to the page, and `WAIT-EVENT`.
//!
//! None of it touches a control's state in the module. A property is a *path*
//! (WASM-ABI-v1 §5) — instance, control, segments with their argument texts —
//! carried to the host in the command batch: written with `SET`, called with
//! `CALL`, read with a query that returns text. A read leaves its answer in a
//! character temporary that behaves as a number when it spells one, which is how
//! `IF NUD-AGE::Value > 17` compares numerically while `MOVE TXT::Text TO
//! WS-NAME` moves characters.

use crate::error::GenResult;
use crate::ir::*;
use crate::lower::Lower;
use cobolt_ast::expr::*;
use cobolt_ast::stmt::*;
use cobolt_wasm_rt::cmd::{CMD_CALL, Q_CALLR, Q_CALLRW, Q_CALLRWS, Q_GET};
use wasm_encoder::Instruction as I;

/// The reserved control ids of ABI v1 §5.
pub const CTRL_FORM: u32 = 0;
pub const CTRL_SUPER: u32 = 1;
pub const CTRL_APP: u32 = 2;
/// A window the program holds a handle to (ABI A40, `ctrl::HANDLE`).
pub const CTRL_HANDLE: u32 = cobolt_wasm_abi::codes::ctrl::HANDLE;
/// The generator's own id for a chain on a windowHandler: `HANDLE_RECV | i`, `i` the receiver's place in
/// `Lower::handle_recvs`. It never reaches the host: the path is sent on [`CTRL_HANDLE`].
const HANDLE_RECV: u32 = 0x4000_0000;

/// Is `ctrl` the generator's id for a windowHandler receiver?
fn is_handle_recv(ctrl: u32) -> bool {
    ctrl & 0xC000_0000 == HANDLE_RECV
}

/// One segment of a member chain: the member's name and its arguments, and
/// whether the source wrote parentheses.
pub(crate) struct Seg<'e> {
    pub name: &'e str,
    pub args: &'e [Expr],
    pub parens: bool,
    /// The part a `Split` answers, from the subscript written after it (`X::Split(",")(2)`): the
    /// first index, as the interpreter reads it; `None` when none is written.
    pub index: Option<&'e Expr>,
}

/// Which arguments of a path's last segment go with every character (`rt_path_arg_whole`), the spaces
/// around their text kept; every other argument is trimmed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Whole {
    None,
    One(usize),
    All,
}

impl From<Option<usize>> for Whole {
    fn from(one: Option<usize>) -> Whole {
        one.map_or(Whole::None, Whole::One)
    }
}

/// `X::Split(sep)` with a subscript after it is the part of that index (the interpreter's `eval_expr`
/// of a subscripted `Split`): is `base` that `Split`?
pub(crate) fn is_split_member(base: &Expr) -> bool {
    matches!(base, Expr::Member { member, .. } if member.eq_ignore_ascii_case("SPLIT"))
}

impl Lower<'_, '_> {
    /// A member chain's root and segments, outermost call last. A subscript after a `Split` is that
    /// segment's index.
    pub(crate) fn chain<'e>(e: &'e Expr) -> Option<(&'e Expr, Vec<Seg<'e>>)> {
        let mut segs = Vec::new();
        let mut cur = e;
        loop {
            let (member_expr, index) = match cur {
                Expr::Subscript { base, indices, .. } if is_split_member(base) && !indices.is_empty() => (base.as_ref(), indices.first()),
                Expr::Member { .. } => (cur, None),
                _ => break,
            };
            let Expr::Member { recv, member, args, parens, .. } = member_expr else { break };
            segs.push(Seg { name: member.as_str(), args: args.as_slice(), parens: *parens, index });
            cur = recv.as_ref();
        }
        if segs.is_empty() {
            return None;
        }
        segs.reverse();
        Some((cur, segs))
    }

    /// The control id a chain's root names, when the root is the form itself,
    /// the form that opened it, the application object or a control of the
    /// form — and `None` when it is a data item (a value method).
    pub(crate) fn host_root(&mut self, root: &Expr) -> Option<u32> {
        let name = match root {
            Expr::Identifier(n, _) => n.to_ascii_uppercase(),
            _ => return None,
        };
        match name.as_str() {
            "ME" | "THIS" => return Some(CTRL_FORM),
            "SUPER" => return Some(CTRL_SUPER),
            "COBOL" => return Some(CTRL_APP),
            _ => {}
        }
        // A data item of the program wins over a control of the same name only
        // when the form has no such control (the codegen names its fields apart).
        if let Some(f) = &self.form {
            if let Some(&id) = f.controls.get(&name) {
                return Some(id);
            }
            if let Some(&id) = f.layers.get(&name) {
                return Some(id);
            }
        }
        None
    }

    /// The receiver a member chain starts with: the host's ([`Self::host_root`]), or a windowHandler,
    /// a `USAGE OBJECT` item naming a window a form opened (`OpenFormAsync … RETURNING`, spec 037 R23).
    /// `first` is the chain's first member: a value method there (`::Trim`) applies to the item's text.
    pub(crate) fn chain_receiver(&mut self, root: &Expr, first: Option<&str>) -> Option<u32> {
        self.host_root(root).or_else(|| self.window_handle_root(root, first))
    }

    fn window_handle_root(&mut self, root: &Expr, first: Option<&str>) -> Option<u32> {
        let Expr::Identifier(written, _) = root else { return None };
        if first.is_some_and(cobolt_ast::methods::is_data_item_method) {
            return None;
        }
        let i = self.layout.lookup(written, &[]).ok()?;
        if !self.layout.items[i].is_window_handle() {
            return None;
        }
        let pos = match self.handle_recvs.iter().position(|(j, _, w)| *j == i && w == written) {
            Some(p) => p,
            None => {
                let opd = self.operand_or_temp(root).ok()?;
                self.handle_recvs.push((i, opd, written.clone()));
                self.handle_recvs.len() - 1
            }
        };
        Some(HANDLE_RECV | pos as u32)
    }

    /// Is `ctrl` one of the form's layers?
    fn is_layer(&self, ctrl: u32) -> bool {
        self.form.as_ref().is_some_and(|f| f.layers.values().any(|&id| id == ctrl))
    }

    /// The rules of spec 091 for a member chain on the host (R19, R37, Q11): a
    /// layer has its properties and nothing else, and `Name` is read only; a
    /// control's `Layer` is read only.
    fn check_layer_rules(&self, root: &Expr, ctrl: u32, segs: &[Seg<'_>], write: bool, line: u32) -> GenResult<()> {
        use crate::error::ErrorKind;
        use cobolt_forms::model::{layer_prop, layer_writable};
        let root_name = match root {
            Expr::Identifier(n, _) => n.as_str(),
            _ => "",
        };
        if self.is_layer(ctrl) {
            let prop = match segs {
                [s] if s.args.is_empty() && !s.parens => s.name,
                _ => {
                    let what = segs.iter().map(|s| s.name).collect::<Vec<_>>().join("::");
                    return self.err_kind(line, ErrorKind::UnknownLayerProperty, what.clone(), format!("{root_name}::{what}: a layer has no members but its properties; a layer has {}", layer_props_text()));
                }
            };
            if !layer_prop(prop) {
                return self.err_kind(line, ErrorKind::UnknownLayerProperty, prop, format!("the layer {root_name} has no property {prop}; a layer has {}", layer_props_text()));
            }
            if write && !layer_writable(prop) {
                return self.err(line, format!("{root_name}::{prop} can be read but not written: a layer is renamed in the designer"));
            }
        } else if write && ctrl >= 3 && segs.last().is_some_and(|s| s.name.eq_ignore_ascii_case("Layer")) {
            return self.err_kind(line, ErrorKind::LayerWrite, format!("{root_name}::Layer"), format!("{root_name}::Layer can be read but not written: a control is moved to another layer in the designer"));
        }
        Ok(())
    }

    /// Is `root` `COBOL::SqlDatabase` in a desktop module, whose runner serves it (DESKTOP-WASM-001
    /// T41d, ABI A44)? A data item of that name wins, as in [`Self::unknown_root`]; a browser
    /// module's database is the backend's, and [`Self::unknown_root`] says so.
    pub(crate) fn is_sql_object(&mut self, root: &Expr) -> bool {
        self.native && matches!(root, Expr::Identifier(n, _) if cobolt_ast::methods::is_sql_database_object(n)) && self.resolve_item(root).is_err()
    }

    /// Is `s` a statement on `COBOL::SqlDatabase` in a desktop module: a verb written as a statement, or
    /// `SET` of one of its properties (T41d)? Such a statement is served in a program with no form too.
    pub(crate) fn on_sql_object(&mut self, s: &Stmt) -> bool {
        let root = match s {
            Stmt::InvokeExpr { expr, .. } => Self::chain(expr).map(|(r, _)| r),
            Stmt::ControlSet { control, .. } => Some(control),
            _ => None,
        };
        root.is_some_and(|r| self.is_sql_object(r))
    }

    /// The one segment of a `COBOL::SqlDatabase` chain the runner serves: the first, when the
    /// segments after it are value methods (`::LastError::Trim()`), which apply to its answer here.
    /// A receiving field (`write`) is that segment alone.
    fn sql_object_segment<'s, 'e>(&self, segs: &'s [Seg<'e>], write: bool, line: u32) -> GenResult<&'s Seg<'e>> {
        if if write { segs.len() == 1 } else { Self::host_prefix_len(segs) == 1 } {
            return Ok(&segs[0]);
        }
        let what = segs.iter().map(|s| s.name).collect::<Vec<_>>().join("::");
        let subject = format!("COBOL::SqlDatabase::{what}");
        self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject}: {}", crate::support::D_SQL_OBJECT_MEMBER))
    }

    /// The operation (`cobolt_wasm_abi::sql::obj`) of `seg`, a member of `COBOL::SqlDatabase`, read as
    /// the interpreter reads it: a bare member is a property, read or (`write`) written; with its
    /// parentheses it is a verb when it has no arguments or names a known method. What the runner does
    /// not serve takes the Cargo path, with the reason.
    fn sql_object_op(&self, seg: &Seg<'_>, write: bool, line: u32) -> GenResult<u32> {
        use crate::support::{SQL_OBJECT_PROPERTIES, SQL_OBJECT_VERBS, SQL_OBJECT_WRITABLE};
        use cobolt_wasm_abi::sql::obj;
        let one_of = |list: &[&str]| list.iter().any(|n| n.eq_ignore_ascii_case(seg.name.trim()));
        let why = if crate::support::is_sql_item_member(seg.name) {
            crate::support::D_SQL_ITEMS
        } else if seg.index.is_some() {
            crate::support::D_SQL_OBJECT_MEMBER
        } else if seg.parens {
            let call = seg.args.is_empty() || cobolt_ast::methods::is_known_method(seg.name);
            if !write && call && one_of(SQL_OBJECT_VERBS) {
                return Ok(obj::CALL);
            }
            crate::support::D_SQL_OBJECT_MEMBER
        } else if write && seg.name.trim().eq_ignore_ascii_case("Mode") {
            crate::support::D_SQL_OBJECT_ASYNC
        } else if write && one_of(SQL_OBJECT_WRITABLE) {
            return Ok(obj::SET);
        } else if !write && one_of(SQL_OBJECT_PROPERTIES) {
            return Ok(obj::GET);
        } else {
            crate::support::D_SQL_OBJECT_MEMBER
        };
        let subject = format!("COBOL::SqlDatabase::{}", seg.name);
        self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject}: {why}"))
    }

    /// `COBOL::SqlDatabase::seg` (T41d): operation `op` with the segment's first argument (a verb reads
    /// no other, as the interpreter's), or with `value`, the operand a write stores. The runner's answer
    /// goes to a new character temporary, whose operand is answered, when `keep`; else it is dropped.
    fn sql_object_run(&mut self, seg: &Seg<'_>, op: u32, value: Option<u32>, keep: bool, line: u32) -> GenResult<Option<u32>> {
        let arg = match (value, seg.args.first()) {
            (Some(v), _) => v,
            (None, Some(a)) if op == cobolt_wasm_abi::sql::obj::CALL => self.operand_or_temp(a)?,
            _ => 0,
        };
        let k = if keep { Some(self.next_ctemp(line)?) } else { None };
        let member = self.pool.opd_lit_alnum(seg.name.trim().as_bytes());
        self.base();
        self.i32c(op as i32);
        self.i32c(member as i32);
        self.i32c(arg as i32);
        self.i32c(k.map_or(cobolt_wasm_rt::state::NCTEMP as i32, |k| k as i32));
        self.call_rt("rt_sql_obj");
        Ok(k.map(|k| self.pool.opd_ctemp(k)))
    }

    /// A chain whose root is neither the host, a control, a layer nor a data
    /// item: on a form that has layers it is most likely a layer's name
    /// mistyped, and the refusal names the layers there are (spec 091 R37).
    fn unknown_root(&mut self, root: &Expr, line: u32) -> Option<crate::error::GenError> {
        let Expr::Identifier(name, _) = root else { return None };
        if self.resolve_item(root).is_ok() {
            return None;
        }
        // `COBOL::SqlDatabase::…` reaches here as the one word the parser makes of it: the database is the Backend's.
        if cobolt_ast::methods::is_sql_database_object(name) {
            return Some(crate::error::GenError::kinded(
                line,
                crate::error::ErrorKind::BackendOnly,
                "COBOL::SqlDatabase",
                crate::support::SQL_DATABASE_OBJECT,
            ));
        }
        let f = self.form.as_ref()?;
        if f.layer_names.is_empty() {
            return None;
        }
        Some(crate::error::GenError::kinded(
            line,
            crate::error::ErrorKind::UnknownLayer,
            name.clone(),
            format!(
                "{name} is not a layer of form {}: its layers are {}; a layer has {}",
                f.name,
                f.layer_names.join(", "),
                layer_props_text()
            ),
        ))
    }

    /// A desktop console module has no form, controls or application object, and a built-in
    /// waits for its own milestone (DESKTOP-WASM-001 T10, T16): a member that reaches the host is
    /// refused there with the reason of its Desktop class, so the Build takes the Cargo path. A
    /// desktop module with forms has them (M2, T30): there only a built-in that waits for a later
    /// milestone than the forms' is refused.
    /// `root` is the chain's first name and `first` its first segment's; on the application object
    /// the segment names the built-in.
    pub(crate) fn desktop_gate(&self, ctrl: u32, root: &Expr, first: Option<&str>, line: u32) -> GenResult<()> {
        if !self.native {
            return Ok(());
        }
        // A desktop module with forms is run by the runner's ABI host, which serves the form, its controls and
        // the application object (M2, T30): only what waits for a later milestone is refused.
        let forms_served = self.form.is_some();
        if forms_served && ctrl != CTRL_APP {
            // The IndexedFile control serves its generated paragraphs' file through the program's own
            // verbs (T39c), and its PartiQL members run in the runner (T48); its binding members, the
            // ones naming a data item, and a statement on a control whose design names one, do not.
            if let (Some(f), Some(member)) = (&self.form, first) {
                let gap = f.indexed_files.contains(&ctrl).then(|| crate::support::partiql_member_gap(member)).flatten().or_else(|| {
                    (f.partiql_items.contains(&ctrl) && crate::support::PARTIQL_ITEM_WRITERS.iter().any(|m| m.eq_ignore_ascii_case(member)))
                        .then_some(crate::support::D_PARTIQL_ITEMS)
                });
                if let Some(why) = gap {
                    let root = match root {
                        Expr::Identifier(n, _) => n.as_str(),
                        _ => "",
                    };
                    let subject = format!("{root}::{member}");
                    return self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject}: {why}"));
                }
                // A SqlDatabase's methods are the runner's (T41c); a member that names one of the
                // program's data items, which the interpreter looks up by name, is not.
                if f.sql_databases.contains(&ctrl) && crate::support::is_sql_item_member(member) {
                    let root = match root {
                        Expr::Identifier(n, _) => n.as_str(),
                        _ => "",
                    };
                    let subject = format!("{root}::{member}");
                    return self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject}: {}", crate::support::D_SQL_ITEMS));
                }
                // A RestClient's requests, and a WebSearch's, are the runner's (T42b, T47a); a member
                // that names one of the program's data items, which the interpreter looks up by name,
                // is not.
                if f.rest_clients.contains(&ctrl) && crate::support::is_rest_item_member(member) {
                    let root = match root {
                        Expr::Identifier(n, _) => n.as_str(),
                        _ => "",
                    };
                    let subject = format!("{root}::{member}");
                    return self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject}: {}", crate::support::D_REST_ITEMS));
                }
                // An AgentObject's `Ask` and declared tools are the runner's (T47c); a member that names
                // one of the program's data items, or offers a tool the runtime answers itself, is not.
                if let Some(why) = f.agents.contains(&ctrl).then(|| crate::support::agent_member_gap(member)).flatten() {
                    let root = match root {
                        Expr::Identifier(n, _) => n.as_str(),
                        _ => "",
                    };
                    let subject = format!("{root}::{member}");
                    return self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject}: {why}"));
                }
            }
            return Ok(());
        }
        let root = match root {
            Expr::Identifier(n, _) => n.as_str(),
            _ => "",
        };
        if ctrl == CTRL_APP {
            if let Some(b) = first.and_then(|n| crate::support::builtin_entry(&n.to_ascii_uppercase())) {
                let why = match b.native {
                    crate::support::Native::Cargo(why) => Some(why),
                    crate::support::Native::Form => (!forms_served).then_some(crate::support::D_FORMS),
                    crate::support::Native::Module | crate::support::Native::Host(_) | crate::support::Native::Files => None,
                };
                return match why {
                    Some(why) => {
                        let subject = format!("COBOL::\"{}\"", b.name);
                        let message = format!("{subject}: {why}");
                        self.err_kind(line, crate::error::ErrorKind::Other, subject, message)
                    }
                    None => Ok(()),
                };
            }
        }
        if forms_served {
            return Ok(());
        }
        let subject = first.map(|n| format!("{root}::{n}")).unwrap_or_else(|| root.to_string());
        self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject}: {}", crate::support::D_FORMS))
    }

    /// Does `e` start a chain that reaches the host: a control, the form, the application object, or
    /// in a desktop module `COBOL::SqlDatabase`, which the runner serves (T41d)?
    pub(crate) fn is_host_member(&mut self, e: &Expr) -> bool {
        match Self::chain(e) {
            Some((root, segs)) => {
                let first = segs.first().map(|s| s.name);
                self.chain_receiver(root, first).is_some() || self.is_sql_object(root)
            }
            None => false,
        }
    }

    /// Evaluate the arguments of every segment (they may read the host or need
    /// temporaries, so they come before the path is started), then start the
    /// path and add each segment with its arguments.
    fn build_path(&mut self, ctrl: u32, segs: &[Seg<'_>]) -> GenResult<()> {
        let mut arg_opds = self.path_args(segs)?;
        let whole = if self.partiql_args(ctrl, segs, &mut arg_opds)? { Whole::All } else { self.agent_file_args(ctrl, segs, &mut arg_opds)?.into() };
        self.emit_path_keeping(ctrl, segs, arg_opds, whole);
        Ok(())
    }

    /// The PartiQL methods of an IndexedFile in a desktop module (DESKTOP-WASM-001 T48, ABI A55)
    /// make the module require `partiql`. `Execute`, `ExecuteTransaction` and `Get` send every
    /// argument with every character, as the interpreter reads them, and one argument more: the kind
    /// of each argument after the statement, one character each (`N` a number, `P` a control's
    /// property, `S` anything else; `cobolt_runtime::partiql::door::ArgKind`), so the runner types
    /// each as the interpreter types it by its declaration (R3). Answers whether the arguments go whole.
    fn partiql_args(&mut self, ctrl: u32, segs: &[Seg<'_>], arg_opds: &mut [Vec<u32>]) -> GenResult<bool> {
        if !self.native || segs.len() != 1 || !self.form.as_ref().is_some_and(|f| f.indexed_files.contains(&ctrl)) {
            return Ok(false);
        }
        let seg = &segs[0];
        let member = seg.name.trim().to_ascii_uppercase();
        if !matches!(member.as_str(), "EXECUTE" | "EXECUTETRANSACTION" | "FETCH" | "GET" | "COLUMNNAMES" | "COLUMNCOUNT") {
            return Ok(false);
        }
        self.names.file_caps.insert(cobolt_wasm_abi::codes::cap::PARTIQL);
        if !matches!(member.as_str(), "EXECUTE" | "EXECUTETRANSACTION" | "GET") {
            return Ok(false);
        }
        // `Execute`'s first argument is the statement, which has no kind.
        let first = usize::from(member != "GET");
        let mut kinds = String::new();
        for (e, &o) in seg.args.iter().zip(arg_opds[0].iter()).skip(first) {
            kinds.push(self.partiql_kind(e, o));
        }
        let span = seg.args.first().map_or_else(|| cobolt_lexer::Span::new(0, 0, 0, 0), |a| a.span());
        let k = self.operand_or_temp(&Expr::Literal(Literal::String(kinds), span))?;
        arg_opds[0].push(k);
        Ok(true)
    }

    /// The kind of one argument of a PartiQL method, as the interpreter types it (`partiql_arg` and
    /// `partiql_note_origins`): a control's property (`P`), whatever it holds; a data item by its
    /// declaration, numeric (`N`) or not (`S`); a reference modification always alphanumeric; a
    /// literal and anything else by its value (`opd`, the operand it was evaluated into).
    fn partiql_kind(&mut self, e: &Expr, opd: u32) -> char {
        fn literal(l: &Literal) -> char {
            match l {
                Literal::Integer(_) | Literal::IntegerDigits(..) | Literal::Float(_) | Literal::Decimal(..) => 'N',
                Literal::Figurative(cobolt_ast::expr::FigurativeConstant::Zero | cobolt_ast::expr::FigurativeConstant::Null) => 'N',
                Literal::Figurative(cobolt_ast::expr::FigurativeConstant::All(inner)) => literal(inner),
                _ => 'S',
            }
        }
        match e {
            Expr::Member { recv, args, parens: false, .. } if args.is_empty() => match recv.as_ref() {
                Expr::Identifier(root, _)
                    if self.form.as_ref().is_some_and(|f| f.controls.contains_key(&root.to_ascii_uppercase())) && self.layout.lookup(root, &[]).is_err() =>
                {
                    'P'
                }
                _ => self.operand_kind(opd),
            },
            Expr::RefMod { .. } => 'S',
            Expr::Identifier(..) | Expr::Qualified { .. } | Expr::Subscript { .. } => {
                if self.is_numeric_item(e) {
                    'N'
                } else {
                    'S'
                }
            }
            Expr::Literal(l, _) => literal(l),
            _ => self.operand_kind(opd),
        }
    }

    /// `N` for an operand that holds a number (a numeric literal, a computed value), `S` otherwise.
    fn operand_kind(&self, opd: u32) -> char {
        let tag = opd.checked_sub(self.pool.base).and_then(|at| self.pool.bytes.get(at as usize)).copied();
        match tag {
            Some(cobolt_wasm_rt::opd::T_LIT_NUM | cobolt_wasm_rt::opd::T_TEMP) => 'N',
            _ => 'S',
        }
    }

    /// An AgentObject's tools that the runner answers itself, in a desktop module (DESKTOP-WASM-001
    /// T47c). `AllowKnowledgeBase` and `DenyKnowledgeBase` (ABI A52) make the module require
    /// `agent.kb`; they name the KnowledgeBase as the program wrote it, which the runner finds in the
    /// form as the interpreter does. The file tools (ABI A51) make it require `agent.files`, and
    /// `AllowFile( fd-name [cidx-path] )` sends two more arguments than
    /// the program wrote, for the runner to lay the file out as the interpreter does from the program's
    /// own `FD`: the file's number in `prfiles`, and the path its `ASSIGN` names now (a data item's
    /// value, sent with every character, or the literal). The argument `cidx-path` is always sent, empty
    /// when not written, so they are the third and the fourth; the answer is which argument is sent
    /// whole.
    ///
    /// The file is named by a literal; one named by a data item, which the interpreter looks up by name
    /// at run time, takes the Cargo path, and so does a file that is not INDEXED, whose `FD` the module
    /// carries no text of. A literal that names no file of the program gets nothing more, and the
    /// runner answers `0`, as the interpreter does.
    fn agent_file_args(&mut self, ctrl: u32, segs: &[Seg<'_>], arg_opds: &mut [Vec<u32>]) -> GenResult<Option<usize>> {
        if !self.native || segs.len() != 1 || !self.form.as_ref().is_some_and(|f| f.agents.contains(&ctrl)) {
            return Ok(None);
        }
        let seg = &segs[0];
        let member = seg.name.trim().to_ascii_uppercase();
        if crate::support::AGENT_KB_MEMBERS.iter().any(|m| m.eq_ignore_ascii_case(&member)) {
            self.names.file_caps.insert(cobolt_wasm_abi::codes::cap::AGENT_KB);
            return Ok(None);
        }
        if !crate::support::AGENT_FILE_MEMBERS.iter().any(|m| m.eq_ignore_ascii_case(&member)) {
            return Ok(None);
        }
        self.names.file_caps.insert(cobolt_wasm_abi::codes::cap::AGENT_FILES);
        if member != "ALLOWFILE" {
            return Ok(None);
        }
        // With no argument the program names no file: the runner answers 0, as the interpreter does.
        let Some(first) = seg.args.first() else {
            return Ok(None);
        };
        let span = first.span();
        let subject = {
            let ctl = self.form.as_ref().and_then(|f| f.controls.iter().find(|(_, &v)| v == ctrl).map(|(k, _)| k.clone())).unwrap_or_default();
            format!("{ctl}::{}", seg.name)
        };
        let fd = match first {
            Expr::Literal(Literal::String(n), _) => n.trim().to_ascii_uppercase(),
            _ => {
                let why = crate::support::D_AGENT_FILE_NAME;
                return self.err_kind(span.line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject}: {why}"));
            }
        };
        let Some(&(id, _)) = self.native_ids.get(&fd) else {
            return Ok(None);
        };
        if self.names.native_files.get(id as usize).is_none_or(|d| d.definition.is_none()) {
            let why = crate::support::D_AGENT_FILE_ORG;
            return self.err_kind(span.line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject}: {why}"));
        }
        let assign = self.native_assigns.get(&fd).cloned().unwrap_or_default();
        let path = match self.layout.lookup(&assign, &[]) {
            Ok(i) => self.item_operand(i, &[], None, span.line)?,
            Err(_) => self.operand_or_temp(&Expr::Literal(Literal::String(assign), span))?,
        };
        let id = self.operand_or_temp(&Expr::Literal(Literal::String(id.to_string()), span))?;
        let args = &mut arg_opds[0];
        args.truncate(2);
        while args.len() < 2 {
            let empty = self.operand_or_temp(&Expr::Literal(Literal::String(String::new()), span))?;
            args.push(empty);
        }
        args.push(id);
        args.push(path);
        Ok(Some(3))
    }

    /// The operands of every segment's arguments, evaluated now.
    pub(crate) fn path_args(&mut self, segs: &[Seg<'_>]) -> GenResult<Vec<Vec<u32>>> {
        let mut arg_opds: Vec<Vec<u32>> = Vec::with_capacity(segs.len());
        for s in segs {
            let mut v = Vec::with_capacity(s.args.len());
            for a in s.args {
                v.push(self.operand_or_temp(a)?);
            }
            arg_opds.push(v);
        }
        Ok(arg_opds)
    }

    /// Start the path and add each segment with its (already evaluated) arguments.
    pub(crate) fn emit_path(&mut self, ctrl: u32, segs: &[Seg<'_>], arg_opds: Vec<Vec<u32>>) {
        self.emit_path_keeping(ctrl, segs, arg_opds, Whole::None);
    }

    /// [`Self::emit_path`], with the arguments of the last segment that `whole` names sent with every
    /// character, the spaces around their text kept (`rt_path_arg_whole`); every other argument is
    /// trimmed.
    pub(crate) fn emit_path_keeping(&mut self, ctrl: u32, segs: &[Seg<'_>], arg_opds: Vec<Vec<u32>>, whole: Whole) {
        if ctrl == CTRL_APP {
            if let Some(first) = segs.first() {
                self.names.builtins.insert(first.name.to_string());
            }
        }
        if is_handle_recv(ctrl) {
            // ABI A40: the first segment names the windowHandler and carries the handle it holds.
            let (_, opd, written) = self.handle_recvs[(ctrl & !HANDLE_RECV) as usize].clone();
            self.i32c(CTRL_HANDLE as i32);
            self.call_rt("rt_path_begin");
            let id = self.names.id(&written);
            self.i32c(id as i32);
            self.call_rt("rt_path_seg");
            self.base();
            self.i32c(opd as i32);
            self.call_rt("rt_path_arg");
        } else {
            self.i32c(ctrl as i32);
            self.call_rt("rt_path_begin");
        }
        let last = segs.len().saturating_sub(1);
        for (k, (s, opds)) in segs.iter().zip(arg_opds).enumerate() {
            let id = self.names.id(s.name);
            self.i32c(id as i32);
            self.call_rt("rt_path_seg");
            for (j, o) in opds.into_iter().enumerate() {
                self.base();
                self.i32c(o as i32);
                let keep = k == last && (whole == Whole::All || whole == Whole::One(j));
                self.call_rt(if keep { "rt_path_arg_whole" } else { "rt_path_arg" });
            }
        }
    }

    /// Ask the host the `super::` reads of `asks` (`GET_WAIT` / `CALLR_WAIT`, the answers in their
    /// character temporaries). The opener may live in another window (ABI A24, WASM-011 R11): the host
    /// then answers each "pending", the task **suspends once, here**, and the same pass runs again after
    /// `pr_resume`, when every answer is at hand. The arguments are evaluated inside the pass, so what
    /// was in the temporaries while other windows' events ran is never read. No statement is run twice:
    /// the pass is placed before the statement that reads (see `lower_super`).
    pub(crate) fn ask_super(&mut self, asks: Vec<crate::lower_super::SuperAsk<'_>>) -> GenResult<()> {
        if asks.is_empty() {
            return Ok(());
        }
        let again = self.new_block("super-ask");
        let waiting = self.new_block("super-wait");
        let answered = self.new_block("super-answered");
        self.jump(again);
        self.begin(again);
        for a in &asks {
            let arg_opds = self.path_args(&a.segs)?;
            self.emit_path(CTRL_SUPER, &a.segs, arg_opds);
            self.i32c(a.op);
            self.i32c(a.k as i32);
            self.i32c(1);
            self.call_rt("rt_cmd_query");
        }
        // Asked of the host, answered "pending" by any of them? Wait for all, then ask again.
        self.call_rt("rt_query_pending");
        self.branch(waiting, answered);
        self.begin(waiting);
        self.suspend_here("super-read");
        self.jump(again);
        self.begin(answered);
        Ok(())
    }

    /// `MOVE x TO Ctrl::Property`: the value goes to the host as text.
    pub(crate) fn member_set(&mut self, target: &Expr, value: u32, line: u32) -> GenResult<()> {
        let Some((root, segs)) = Self::chain(target) else {
            return self.err(line, "a control property was expected as the receiving field");
        };
        let Some(ctrl) = self.host_root(root) else {
            if self.is_sql_object(root) {
                let seg = self.sql_object_segment(&segs, true, line)?;
                let op = self.sql_object_op(seg, true, line)?;
                self.sql_object_run(seg, op, Some(value), false, line)?;
                return Ok(());
            }
            if let Some(e) = self.unknown_root(root, line) {
                return Err(e);
            }
            return self.err(line, "a value method cannot be a receiving field");
        };
        self.desktop_gate(ctrl, root, segs.first().map(|s| s.name), line)?;
        self.check_layer_rules(root, ctrl, &segs, true, line)?;
        self.build_path(ctrl, &segs)?;
        self.base();
        self.i32c(value as i32);
        self.call_rt("rt_cmd_set");
        Ok(())
    }

    /// A control property as the receiving field of `ADD`, `SUBTRACT`, `MULTIPLY`, `DIVIDE` or
    /// `COMPUTE` (`ADD 1 TO NUD-1::Value`): the result on the top register goes into a temporary and
    /// from there to the host as text, as `MOVE <expression> TO Ctrl::Property` does. `operand`
    /// reads a property; storing into what it answers raised COBOL run-time error 104 (a bad
    /// operand) on the page. `dup` keeps the result for the next receiver.
    pub(crate) fn store_to_member(&mut self, target: &Expr, rounded: bool, dup: bool) -> GenResult<()> {
        let line = target.span().line;
        if rounded {
            return self.err(line, "ROUNDED is not supported with a control property as the receiving field");
        }
        let k = self.temp_next;
        if k >= cobolt_wasm_rt::state::NTEMP as u32 {
            return self.err(line, "this statement needs more than eight computed operands");
        }
        self.temp_next += 1;
        if dup {
            self.i32c(0);
            self.call_rt("rt_pick");
        }
        self.i32c(k as i32);
        self.call_rt("rt_save_temp");
        let src = self.pool.opd_temp(k);
        self.member_set(target, src, line)
    }

    /// A control property as the receiving field of `STRING`: its current text
    /// in a character temporary widened to the temporary's full capacity, so
    /// the statement can write into it. Answers the temporary's index and its
    /// operand; the caller trims it (`rt_ctemp_rtrim`) and writes it back with
    /// [`Self::member_set`].
    pub(crate) fn member_receiver(&mut self, target: &Expr, line: u32) -> GenResult<(u32, u32)> {
        let Some((root, segs)) = Self::chain(target) else {
            return self.err(line, "a control property was expected as the receiving field");
        };
        let Some(ctrl) = self.host_root(root) else {
            if self.is_sql_object(root) {
                let subject = format!("COBOL::SqlDatabase::{}", segs[0].name);
                return self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject}: {}", crate::support::D_SQL_OBJECT_MEMBER));
            }
            if let Some(e) = self.unknown_root(root, line) {
                return Err(e);
            }
            return self.err(line, "a value method cannot be a receiving field");
        };
        self.desktop_gate(ctrl, root, segs.first().map(|s| s.name), line)?;
        self.check_layer_rules(root, ctrl, &segs, true, line)?;
        let k = self.ctemp_next;
        let opd = self.host_read(ctrl, &segs, line)?;
        self.i32c(k as i32);
        self.call_rt("rt_ctemp_widen");
        Ok((k, opd))
    }

    /// The display-list opcode of `ctrl::Method(args)` when `ctrl` is a Canvas2D and the method is one
    /// of the commands the routines encode (every operand a number, colour or text; the count right).
    /// Anything else stays a host call, which answers it as it always has.
    fn canvas_command(&self, ctrl: u32, segs: &[Seg<'_>]) -> Option<u8> {
        let f = self.form.as_ref()?;
        if !f.canvases.contains(&ctrl) || segs.len() != 1 {
            return None;
        }
        let s = &segs[0];
        let info = cobolt_graphics_wire::COMMANDS.iter().find(|c| c.opcode < 0x80 && c.name.eq_ignore_ascii_case(s.name))?;
        let scalar = info.operands.iter().all(|(_, k)| matches!(*k, "u8" | "u32" | "f32" | "rgba" | "str"));
        (scalar && s.args.len() == info.operands.len()).then_some(info.opcode)
    }

    /// Before a call or a read on a graphics object other than a Canvas2D (`FormCtx::graphics`),
    /// the display list waiting for a canvas goes to the host (`rt_gfx_send`, ABI A23). The object's
    /// method may draw on the canvas by its handle (`PLT-1::RenderToCanvas( WS-CNV 0 0 320 300 )`),
    /// and it must find there what the program drew before it, as under the interpreter: kept back,
    /// a `Clear` drawn before the call would run after it and wipe out what it drew.
    fn canvas_drawing_first(&mut self, ctrl: u32) {
        if self.form.as_ref().is_some_and(|f| !f.canvases.is_empty() && f.graphics.contains(&ctrl)) {
            self.call_rt("rt_gfx_send");
        }
    }

    /// `Ctrl::Method(args)` as a statement: the call, its result discarded.
    pub(crate) fn member_call(&mut self, e: &Expr, line: u32) -> GenResult<()> {
        let Some((root, segs)) = Self::chain(e) else {
            return self.err(line, "a control member was expected");
        };
        let Some(ctrl) = self.chain_receiver(root, segs.first().map(|s| s.name)) else {
            // `COBOL::SqlDatabase::Close()`: the verb runs, its answer (and any value method after it) dropped.
            if self.is_sql_object(root) {
                let seg = self.sql_object_segment(&segs, false, line)?;
                let op = self.sql_object_op(seg, false, line)?;
                self.sql_object_run(seg, op, None, false, line)?;
                return Ok(());
            }
            if let Some(e) = self.unknown_root(root, line) {
                return Err(e);
            }
            // A value method used as a statement has no effect.
            return Ok(());
        };
        self.desktop_gate(ctrl, root, segs.first().map(|s| s.name), line)?;
        self.check_layer_rules(root, ctrl, &segs, false, line)?;
        // A Canvas2D drawing method is one command of a display list, not a host call (ABI A23).
        if let Some(op) = self.canvas_command(ctrl, &segs) {
            self.build_path(ctrl, &segs)?;
            self.i32c(op as i32);
            self.call_rt("rt_gfx_cmd");
            return Ok(());
        }
        self.canvas_drawing_first(ctrl);
        self.build_path(ctrl, &segs)?;
        self.call_rt("rt_cmd_call");
        if let Some(last) = segs.last() {
            self.suspend_if_waiting(ctrl, last.name);
        }
        let _ = CMD_CALL;
        Ok(())
    }

    /// `Ctrl::Property` or `Ctrl::Method(args)` as a value: the host's answer
    /// text, in a character temporary.
    pub(crate) fn member_read(&mut self, e: &Expr, line: u32) -> GenResult<u32> {
        let Some((root, segs)) = Self::chain(e) else {
            return self.err(line, "a control member was expected");
        };
        let Some(ctrl) = self.chain_receiver(root, segs.first().map(|s| s.name)) else {
            return self.err(line, "not a control member");
        };
        self.host_read(ctrl, &segs, line)
    }

    /// How many leading segments of a chain after a control are the host's: up to the first value
    /// method (`Trim`, `Upper`, …) after the first segment, which applies to the answer here.
    pub(crate) fn host_prefix_len(segs: &[Seg<'_>]) -> usize {
        use cobolt_ast::methods::is_data_item_method;
        segs.iter().skip(1).position(|s| is_data_item_method(s.name)).map_or(segs.len(), |i| i + 1)
    }

    /// Read `ctrl::segs…` from the host into a new character temporary.
    fn host_read(&mut self, ctrl: u32, segs: &[Seg<'_>], line: u32) -> GenResult<u32> {
        let k = self.next_ctemp(line)?;
        self.canvas_drawing_first(ctrl);
        self.build_path(ctrl, segs)?;
        let last_parens = segs.last().map(|s| s.parens).unwrap_or(false);
        self.i32c(if last_parens { Q_CALLR as i32 } else { Q_GET as i32 });
        self.i32c(k as i32);
        self.i32c(1);
        self.call_rt("rt_cmd_query");
        Ok(self.pool.opd_ctemp(k))
    }

    pub(crate) fn next_ctemp(&mut self, line: u32) -> GenResult<u32> {
        let k = self.ctemp_next;
        if k >= cobolt_wasm_rt::state::NCTEMP as u32 {
            return self.too_many_reads(line, "this statement reads more than four properties or character-valued functions; split it into statements that read at most four");
        }
        self.ctemp_next += 1;
        Ok(k)
    }

    /// A member chain as a value (WASM-010 T11). As the interpreter's
    /// `eval_member` does, a value method (`Trim`, `Upper`, `Lower`, `Replace`,
    /// `Len`, `Split` and their older spellings, `cobolt_ast::methods`) applies
    /// to the value before it — a data item, a literal, or what the host
    /// answered for the segments before it; the segments up to the first value
    /// method after a control are the host's.
    pub(crate) fn member_value(&mut self, e: &Expr, line: u32) -> GenResult<u32> {
        let Some((root, segs)) = Self::chain(e) else {
            return self.err(line, "a member was expected");
        };
        let (mut recv, local) = match self.chain_receiver(root, segs.first().map(|s| s.name)) {
            Some(ctrl) => {
                self.desktop_gate(ctrl, root, segs.first().map(|s| s.name), line)?;
                let host_len = Self::host_prefix_len(&segs);
                self.check_layer_rules(root, ctrl, &segs[..host_len], false, line)?;
                // A `super::` read the statement asked for before it began (`lower_super`): the answer is there.
                let r = match self.hoisted_super_read(e) {
                    Some(k) => self.pool.opd_ctemp(k),
                    None => self.host_read(ctrl, &segs[..host_len], line)?,
                };
                if host_len == segs.len() {
                    return Ok(r);
                }
                (r, &segs[host_len..])
            }
            // `COBOL::SqlDatabase::Query(…)`: the runner's answer, then the value methods after it.
            None if self.is_sql_object(root) => {
                let seg = self.sql_object_segment(&segs, false, line)?;
                let op = self.sql_object_op(seg, false, line)?;
                let r = self.sql_object_run(seg, op, None, true, line)?.expect("the answer is kept");
                if segs.len() == 1 {
                    return Ok(r);
                }
                (r, &segs[1..])
            }
            None => {
                if let Some(e) = self.unknown_root(root, line) {
                    return Err(e);
                }
                (self.operand_or_temp(root)?, &segs[..])
            }
        };
        let mut k = None;
        for s in local {
            let Some(code) = value_method_code(s.name) else {
                return self.err(
                    line,
                    format!("{} is not a value method of a data item: a data item answers {}", s.name, cobolt_ast::methods::DATA_ITEM_METHODS_TEXT),
                );
            };
            if code == cobolt_wasm_rt::strings::VM_LEN && s.parens {
                return self.err(
                    line,
                    format!("{} is a property of a data item, not a method: write it without the parentheses", s.name),
                );
            }
            let a = match s.args.first() {
                Some(x) => self.operand_or_temp(x)?,
                None => 0,
            };
            // `Split` reads its separator alone (a second argument is ignored, as the interpreter
            // ignores it); its second operand is the part's index, when one is written after it.
            let second = if code == cobolt_wasm_rt::strings::VM_SPLIT { s.index } else { s.args.get(1) };
            let b = match second {
                Some(x) => self.operand_or_temp(x)?,
                None => 0,
            };
            // Every method of the chain writes the same temporary: the routine
            // reads its operands before it writes the result.
            let slot = match k {
                Some(k) => k,
                None => {
                    let n = self.next_ctemp(line)?;
                    k = Some(n);
                    n
                }
            };
            self.base();
            self.i32c(code as i32);
            self.i32c(recv as i32);
            self.i32c(a as i32);
            self.i32c(b as i32);
            self.i32c(slot as i32);
            self.call_rt("rt_value_method");
            recv = self.pool.opd_ctemp(slot);
        }
        Ok(recv)
    }

    /// `INVOKE Ctrl 'Method' USING args [RETURNING x] [ON EXCEPTION …] [NOT ON EXCEPTION …]`.
    pub(crate) fn invoke_(
        &mut self,
        object: &str,
        method: &str,
        args: &[Expr],
        returning: Option<&Expr>,
        (on_exc, not_exc): (&[Stmt], &[Stmt]),
        line: u32,
    ) -> GenResult<()> {
        let root = Expr::Identifier(object.to_string(), cobolt_lexer::Span::new(0, 0, line, 0));
        let Some(ctrl) = self.chain_receiver(&root, Some(method)) else {
            return self.err(line, format!("INVOKE {object}: it is not a control of this form"));
        };
        let seg = Seg { name: method, args, parens: true, index: None };
        if !on_exc.is_empty() || !not_exc.is_empty() {
            if ctrl != CTRL_SUPER || self.form.is_none() {
                return self.err(
                    line,
                    "ON EXCEPTION / NOT ON EXCEPTION on an INVOKE is available for `super::` in a WebAssembly module (the window that opened this one may be closed); a control or the form answers at once, and what it refuses is a run-time error",
                );
            }
            return self.invoke_super_guarded(seg, returning, on_exc, not_exc, line);
        }
        self.canvas_drawing_first(ctrl);
        match returning {
            None => {
                self.build_path(ctrl, std::slice::from_ref(&seg))?;
                self.call_rt("rt_cmd_call");
                self.suspend_if_waiting(ctrl, method);
            }
            Some(r) => {
                let k = self.ctemp_next;
                if k >= cobolt_wasm_rt::state::NCTEMP as u32 {
                    return self.too_many_reads(line, "this statement reads more than four properties or character-valued functions; split it into statements that read at most four");
                }
                self.ctemp_next += 1;
                if ctrl == CTRL_SUPER && self.form.is_some() {
                    // The answer may come from another window: ask, wait, ask again (`ask_super`).
                    self.ask_super(vec![crate::lower_super::SuperAsk { segs: vec![seg], op: Q_CALLRW as i32, k }])?;
                } else {
                    self.build_path(ctrl, std::slice::from_ref(&seg))?;
                    self.i32c(Q_CALLR as i32);
                    self.i32c(k as i32);
                    self.i32c(1);
                    self.call_rt("rt_cmd_query");
                }
                let src = self.pool.opd_ctemp(k);
                self.move_operand_to(src, r, line)?;
                self.suspend_if_waiting(ctrl, method);
            }
        }
        Ok(())
    }

    /// `INVOKE super::Method(…) [RETURNING x] ON EXCEPTION … [NOT ON EXCEPTION …]` (ABI A25, WASM-011 Q3).
    ///
    /// The window that opened this one may have been closed, or never answer: the read is asked soft
    /// (`Q_CALLRWS`), the host reports no run-time error and the module asks `rt_query_failed`. A failed
    /// read runs `ON EXCEPTION` and leaves `x` as it was; one that went through stores its answer in `x`
    /// and runs `NOT ON EXCEPTION`. Without `RETURNING` the answer is dropped, the call is still waited for.
    fn invoke_super_guarded(
        &mut self,
        seg: Seg<'_>,
        returning: Option<&Expr>,
        on_exc: &[Stmt],
        not_exc: &[Stmt],
        line: u32,
    ) -> GenResult<()> {
        let k = self.ctemp_next;
        if k >= cobolt_wasm_rt::state::NCTEMP as u32 {
            return self.too_many_reads(line, "this statement reads more than four properties or character-valued functions; split it into statements that read at most four");
        }
        self.ctemp_next += 1;
        self.ask_super(vec![crate::lower_super::SuperAsk { segs: vec![seg], op: Q_CALLRWS as i32, k }])?;
        let failed = self.new_block("super-failed");
        let went_through = self.new_block("super-went-through");
        let join = self.new_block("end-invoke");
        self.call_rt("rt_query_failed");
        self.branch(failed, went_through);
        self.begin(went_through);
        if let Some(r) = returning {
            let src = self.pool.opd_ctemp(k);
            self.move_operand_to(src, r, line)?;
        }
        self.stmts(not_exc)?;
        if self.cur.is_some() {
            self.jump(join);
        }
        self.begin(failed);
        self.stmts(on_exc)?;
        if self.cur.is_some() {
            self.jump(join);
        }
        self.begin(join);
        Ok(())
    }

    /// `MOVE <operand> TO <target>` where the sender is already an operand.
    pub(crate) fn move_operand_to(&mut self, src: u32, to: &Expr, line: u32) -> GenResult<()> {
        if matches!(to, Expr::Member { .. }) {
            return self.member_set(to, src, line);
        }
        let dst = self.receiver(to)?;
        self.base();
        self.i32c(src as i32);
        self.i32c(dst as i32);
        self.call_rt("rt_move");
        Ok(())
    }

    /// `SHOW` / `HIDE` / `CLOSE` of the form: a call on the form itself.
    pub(crate) fn window_op(&mut self, op: &WindowOperation, line: u32) -> GenResult<()> {
        let (name, who) = match op {
            WindowOperation::Show(w) => ("Show", w),
            WindowOperation::Hide(w) => ("Hide", w),
            WindowOperation::Close(w) => ("Close", w),
        };
        let _ = who;
        let seg = Seg { name, args: &[], parens: true, index: None };
        self.build_path(CTRL_FORM, std::slice::from_ref(&seg))?;
        self.call_rt("rt_cmd_call");
        let _ = line;
        Ok(())
    }

    /// `COBOL::"WAIT-EVENT" ( event-id control-id )`: the form is idle. The host
    /// gets what the handler changed, the module stops where it is, and
    /// `pr_event` later writes the event's names into the two items and
    /// continues here.
    pub(crate) fn wait_event(&mut self, using: &[CallArg], line: u32) -> GenResult<()> {
        let mut opds = [0u32; 2];
        for (i, a) in using.iter().take(2).enumerate() {
            let e = match a {
                CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e,
            };
            opds[i] = self.operand(e)?.ok_or_else(|| crate::error::GenError::new(line, "WAIT-EVENT takes data items"))?;
        }
        let slot = match self.park_slot {
            Some(s) => s,
            None => {
                let s = self.layout.alloc_hidden(8);
                self.park_slot = Some(s);
                s
            }
        };
        self.base();
        self.i32c(slot as i32);
        self.i32c(opds[0] as i32);
        self.i32c(opds[1] as i32);
        self.call_rt("rt_park");
        self.call_rt("rt_flush");
        let k = self.new_block("after-wait");
        self.emit(Ins::Pc(k));
        self.emit(Ins::GlobalSet(G::Resume));
        self.i32c(ret::PARK);
        self.finish_raw();
        self.begin(k);
        Ok(())
    }

    /// The control a built-in's first argument names: a literal holding a
    /// control's name (as generated code writes it), or the control itself.
    fn literal_control(&mut self, builtin: &str, obj: &Expr, line: u32) -> GenResult<u32> {
        let name = match obj {
            Expr::Literal(Literal::String(o), _) => o.trim().to_string(),
            Expr::Identifier(o, _) => o.clone(),
            _ => return self.err(line, format!("COBOL::\"{builtin}\" in a browser module names the control in a literal")),
        };
        let root = Expr::Identifier(name.clone(), cobolt_lexer::Span::new(0, 0, line, 0));
        match self.host_root(&root) {
            Some(ctrl) => Ok(ctrl),
            None => self.err(line, format!("COBOL::\"{builtin}\": {name} is not a control of this form")),
        }
    }

    /// `COBOL::"INIT-FORM"` in a desktop module whose form holds IndexedFile controls (DESKTOP-WASM-001
    /// T48, ABI A55): each control, in the order of the manifest, hears `_START` with the program's
    /// `SELECT`s, their names and the `ASSIGN`s as written (each list TAB-separated), as the interpreter
    /// reads them at `COBOL-INIT-FORM`: the runner opens a control whose `AutoOpen` is on when the
    /// program does not declare its file (whose generated paragraphs open it otherwise), and knows
    /// which files the program drives with its verbs, for `AutoCommit`.
    fn partiql_start(&mut self, line: u32) -> GenResult<()> {
        let mut ctrls: Vec<u32> = match &self.form {
            Some(f) if self.native => f.indexed_files.iter().copied().collect(),
            _ => return Ok(()),
        };
        if ctrls.is_empty() {
            return Ok(());
        }
        ctrls.sort_unstable();
        let (mut names, mut assigns) = (Vec::new(), Vec::new());
        if let Some(io) = self.prog.environment.as_ref().and_then(|e| e.input_output.as_ref()) {
            for fc in &io.file_controls {
                names.push(fc.name.clone());
                assigns.push(fc.assign.clone());
            }
        }
        self.names.file_caps.insert(cobolt_wasm_abi::codes::cap::PARTIQL);
        let span = cobolt_lexer::Span::new(0, 0, line, 0);
        let args = [Expr::Literal(Literal::String(names.join("\t")), span), Expr::Literal(Literal::String(assigns.join("\t")), span)];
        // Asked, not queued: the interpreter resolves each control's file here, before the program's
        // `OPEN` (a direct import) can publish its FD definition into the file, so the start must
        // reach the host in program order. The answer is empty and unused.
        let k = self.ctemp_next;
        if k >= cobolt_wasm_rt::state::NCTEMP as u32 {
            return self.too_many_reads(line, "this statement reads more than four properties or character-valued functions; split it into statements that read at most four");
        }
        for ctrl in ctrls {
            let seg = Seg { name: "_START", args: &args, parens: true, index: None };
            let arg_opds = self.path_args(std::slice::from_ref(&seg))?;
            self.emit_path_keeping(ctrl, std::slice::from_ref(&seg), arg_opds, Whole::All);
            self.i32c(Q_CALLR as i32);
            self.i32c(k as i32);
            self.i32c(1);
            self.call_rt("rt_cmd_query");
        }
        Ok(())
    }

    /// A built-in `CALL "COBOL-NAME"` that belongs to the page.
    pub(crate) fn builtin_call(&mut self, name: &str, using: &[CallArg], line: u32) -> GenResult<()> {
        let app = Expr::Identifier("COBOL".to_string(), cobolt_lexer::Span::new(0, 0, line, 0));
        self.desktop_gate(CTRL_APP, &app, Some(name), line)?;
        match name {
            // The page builds the form when it opens it; a desktop form's IndexedFile controls hear
            // that it starts (T48).
            "INIT-FORM" => self.partiql_start(line),
            "WAIT-EVENT" => self.wait_event(using, line),
            // `COBOL-SET-PROPERTY obj prop value` / `COBOL-GET-PROPERTY obj prop dest`
            // name the control and the property in literals; SET-PROPERTY may
            // also take the property's name from a data item, as the
            // interpreter does (WASM-005 T14: the backend's answer names the
            // properties it set at run time) — ABI amendment A14's SET_NAMED.
            "SET-PROPERTY" | "GET-PROPERTY" if using.len() >= 3 => {
                let ex = |a: &CallArg| match a {
                    CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
                };
                let (obj, prop, third) = (ex(&using[0]), ex(&using[1]), ex(&using[2]));
                if name == "SET-PROPERTY" && !matches!(prop, Expr::Literal(Literal::String(_), _)) {
                    let ctrl = self.literal_control(name, &obj, line)?;
                    let p = self.operand_or_temp(&prop)?;
                    let v = self.operand_or_temp(&third)?;
                    self.base();
                    self.i32c(ctrl as i32);
                    self.i32c(p as i32);
                    self.i32c(v as i32);
                    self.call_rt("rt_cmd_set_named");
                    return Ok(());
                }
                let (Expr::Literal(Literal::String(o), _), Expr::Literal(Literal::String(p), _)) = (&obj, &prop) else {
                    return self.err(line, format!("COBOL::\"{name}\" in a browser module names the control and the property in literals"));
                };
                let root = Expr::Identifier(o.clone(), cobolt_lexer::Span::new(0, 0, line, 0));
                let Some(ctrl) = self.host_root(&root) else {
                    return self.err(line, format!("COBOL::\"{name}\": {o} is not a control of this form"));
                };
                let seg = Seg { name: p.as_str(), args: &[], parens: false, index: None };
                if name == "SET-PROPERTY" {
                    let v = self.operand_or_temp(&third)?;
                    self.build_path(ctrl, std::slice::from_ref(&seg))?;
                    self.base();
                    self.i32c(v as i32);
                    self.call_rt("rt_cmd_set");
                } else {
                    let k = self.ctemp_next;
                    if k >= cobolt_wasm_rt::state::NCTEMP as u32 {
                        return self.too_many_reads(line, "this statement reads more than four properties or character-valued functions; split it into statements that read at most four");
                    }
                    self.ctemp_next += 1;
                    self.build_path(ctrl, std::slice::from_ref(&seg))?;
                    self.i32c(Q_GET as i32);
                    self.i32c(k as i32);
                    self.i32c(1);
                    self.call_rt("rt_cmd_query");
                    let src = self.pool.opd_ctemp(k);
                    self.move_operand_to(src, &third, line)?;
                }
                Ok(())
            }
            // `COBOL::"RAISE-EVENT" ( control event [argument] )`: the page
            // queues the event as the control would raise it (A14's RAISE).
            "RAISE-EVENT" if using.len() >= 2 => {
                let ex = |a: &CallArg| match a {
                    CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
                };
                let ctrl = self.literal_control(name, &ex(&using[0]), line)?;
                let ev = self.operand_or_temp(&ex(&using[1]))?;
                let arg = match using.get(2) {
                    Some(a) => self.operand_or_temp(&ex(a))?,
                    None => self.operand_or_temp(&Expr::Literal(Literal::String(String::new()), cobolt_lexer::Span::new(0, 0, line, 0)))?,
                };
                self.base();
                self.i32c(ctrl as i32);
                self.i32c(ev as i32);
                self.i32c(arg as i32);
                self.call_rt("rt_cmd_raise");
                Ok(())
            }
            // `COBOL::"WS-LAST-ERROR" ( class [code [message]] )`: how the last
            // remote call failed, kept by the module from the error frame.
            "WS-LAST-ERROR" => {
                if using.len() > 3 {
                    return self.err(line, "COBOL::\"WS-LAST-ERROR\" in a browser module answers the class, the code and the message; the detail stays with the backend's log");
                }
                let mut opds = [0u32; 3];
                for (i, a) in using.iter().take(3).enumerate() {
                    let e = match a {
                        CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e,
                    };
                    opds[i] = self.receiver(e)?;
                }
                self.base();
                for o in opds {
                    self.i32c(o as i32);
                }
                self.call_rt("rt_ws_last_error");
                Ok(())
            }
            // On the desktop the runner answers the file dialogs (T32); a browser module refuses them below.
            dialog if self.native && crate::lower_remote::is_desktop_dialog(dialog) => self.desktop_file_dialog(dialog, using, line),
            // On the desktop the runner plays the sounds (T49).
            audio if self.native && crate::lower_remote::is_desktop_audio(audio) => self.desktop_audio(audio, using, line),
            // On the desktop the runner keeps an AWS connection's keys (T48).
            aws if self.native && crate::lower_remote::is_desktop_aws(aws) => self.desktop_aws(aws, using, line),
            // On the desktop the runner answers the model list, its keys and the providers (T48).
            models if self.native && cobolt_wasm_abi::codes::model_verb(models).is_some() => {
                let (min, writes) = cobolt_wasm_abi::codes::model_verb(models).expect("one of the model-list built-ins");
                let why = "the runner serves the model list, its keys and the providers in a form's handler only";
                self.desktop_app_builtin(models, using, line, cobolt_wasm_abi::codes::cap::MODELS, min, writes, why)
            }
            // On the desktop the runner searches the files the form's AgentObjects allowed (T48).
            "MCP-SEARCH" if self.native => {
                let why = "the runner searches the files a form's AgentObjects allowed, in a form's handler only";
                self.desktop_app_builtin(name, using, line, cobolt_wasm_abi::codes::cap::MCP_SEARCH, 3, &[2], why)
            }
            // On the desktop the runner writes a built-in's text file and reads and fits its pictures (T39c).
            "APPEND-FILE" | "WRITE-FILE" if self.native => self.desktop_text_write(name, using, line),
            "IMAGE-FIT" | "IMAGE-INFO" if self.native => self.desktop_image(name, using, line),
            // On the desktop the runner keeps the SQL built-ins' connections (T41b).
            "OPEN-DB" | "EXEC-SQL" | "FETCH-ROW" | "NEXT-ROW" | "ROW-COUNT" | "CLOSE-DB" if self.native => self.desktop_sql_db(name, using, line),
            // On the desktop the runner keeps each window's HTTP client and its headers (T42).
            "HTTP-GET" | "HTTP-POST" | "HTTP-PUT" | "HTTP-DELETE" | "HTTP-SET-HEADER" | "HTTP-CLEAR-HEADERS" | "HTTP-DOWNLOAD" if self.native => {
                self.desktop_http(name, using, line)
            }
            // On the desktop the module keeps every file's last status (T39c).
            "FILE-STATUS" if self.native => self.desktop_file_status(using, line),
            // On the desktop the runner runs PartiQL on the program's own open files (T48).
            "PARTIQL" | "PARTIQL-FETCH" | "PARTIQL-ERROR" if self.native => self.desktop_partiql(name, using, line),
            // The page's own verbs (ABI §12.1) are not the desktop's: under the interpreter a form's
            // windows are `ME`'s and a handle's, and a desktop form has no message box (T6, T32). A
            // desktop module refuses them, so the Build takes the Cargo path, which reports them as the
            // interpreter does.
            page if self.native && crate::lower_remote::is_app_verb(page) && !crate::lower_remote::is_mobile_verb(page) => {
                let subject = format!("COBOL::\"{page}\"");
                self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject} is a verb of a web page's application object, which a desktop program does not have"))
            }
            other => {
                // Any other service the page offers is a call on the application
                // object with the arguments as text.
                let args: Vec<Expr> = using
                    .iter()
                    .map(|a| match a {
                        CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
                    })
                    .collect();
                if crate::lower_remote::is_mobile_verb(other) && !self.mobile {
                    let verb = match other {
                        "SECURESTORE" => "SecureStore",
                        "SECURELOAD" => "SecureLoad",
                        _ => "SecureDelete",
                    };
                    return self.err_kind(
                        line,
                        crate::error::ErrorKind::Unsupported,
                        format!("COBOL::{verb}"),
                        format!("COBOL::{verb} is available in a Mobile project only: it keeps a secret in the phone's own secure store, which a web page does not have"),
                    );
                }
                match crate::support::builtin_support(other) {
                    // A verb of the application object the page implements (ABI §12.1).
                    _ if crate::lower_remote::is_app_verb(other) => {}
                    crate::support::Support::Module | crate::support::Support::Conformance => {}
                    crate::support::Support::BackendOnly(alt) | crate::support::Support::MemoryFiles(alt) => {
                        return self.err_kind(line, crate::error::ErrorKind::BackendOnly, format!("COBOL::\"{other}\""), format!("COBOL::\"{other}\" runs on the backend, not in a browser module: {alt}"));
                    }
                    crate::support::Support::Unsupported(why) => {
                        return self.err_kind(line, crate::error::ErrorKind::Unsupported, format!("COBOL::\"{other}\""), format!("COBOL::\"{other}\" is not available in a browser module: {why}"));
                    }
                }
                // `COBOL::SecureLoad( key value )` (A26): the value the shell found
                // lands in the second argument once the task resumes.
                if other == "SECURELOAD" && args.len() > 2 {
                    return self.err(line, "COBOL::SecureLoad takes the key and, optionally, the data item that receives the value");
                }
                let receiver = if other == "SECURELOAD" { args.get(1).cloned() } else { None };
                let sent: &[Expr] = if receiver.is_some() { &args[..1] } else { &args };
                let seg = Seg { name: other, args: sent, parens: true, index: None };
                // `COBOL::"CHART-SET-TABLE"` reads its table at fixed offsets: the table goes whole,
                // the spaces around its text kept, as the interpreter reads the item.
                let whole = (other == "CHART-SET-TABLE").then_some(1);
                let arg_opds = self.path_args(std::slice::from_ref(&seg))?;
                self.emit_path_keeping(CTRL_APP, std::slice::from_ref(&seg), arg_opds, whole.into());
                self.call_rt("rt_cmd_call");
                self.suspend_if_waiting(CTRL_APP, other);
                if let Some(dest) = receiver {
                    self.app_property_into("SecureValue", &dest, line)?;
                }
                Ok(())
            }
        }
    }

    /// The desktop's file dialogs (DESKTOP-WASM-001 T32, ABI A33): `COBOL::"OPEN-FILE-DIALOG"`,
    /// `"SAVE-FILE-DIALOG"` and `"FOLDER-DIALOG"`, whose arguments are the interpreter's (the
    /// title, the filter, the file name, the start folder, and always last the item that receives
    /// the path). The arguments before the last go to the runner as text; the task waits while the
    /// operator chooses; then the last receives the application's `DialogPath`, spaces when the
    /// operator cancelled, as the interpreter moves the answer.
    fn desktop_file_dialog(&mut self, name: &str, using: &[CallArg], line: u32) -> GenResult<()> {
        let subject = format!("COBOL::\"{name}\"");
        if using.len() < 2 {
            return self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject} takes the title and, last, the data item that receives the path"));
        }
        if self.form.is_none() {
            return self.err_kind(
                line,
                crate::error::ErrorKind::Other,
                subject.clone(),
                format!("{subject} waits while the operator chooses, which a desktop module does in a form's handler only"),
            );
        }
        let args: Vec<Expr> = using
            .iter()
            .map(|a| match a {
                CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
            })
            .collect();
        let (sent, dest) = args.split_at(args.len() - 1);
        let seg = Seg { name, args: sent, parens: true, index: None };
        self.build_path(CTRL_APP, std::slice::from_ref(&seg))?;
        self.call_rt("rt_cmd_call");
        self.suspend_if_waiting(CTRL_APP, name);
        self.app_property_into("DialogPath", &dest[0], line)
    }

    /// The desktop's sound player (DESKTOP-WASM-001 T49, ABI A36): `COBOL::"AUDIO-PLAY" ( file
    /// [status] )`, `"AUDIO-PAUSE"`, `"AUDIO-RESUME"` and `"AUDIO-STOP"` ( [status] ), and
    /// `"AUDIO-STATE" ( state [position-ms] [duration-ms] )`, with the interpreter's arguments. The
    /// file goes to the runner as text; the runner answers at once, and each receiver gets its answer
    /// from the application object: `AudioStatus`, or `AudioState`, `AudioPosition` and
    /// `AudioDuration`. Arguments after those the interpreter reads are ignored, as it ignores them.
    fn desktop_audio(&mut self, name: &str, using: &[CallArg], line: u32) -> GenResult<()> {
        let subject = format!("COBOL::\"{name}\"");
        if self.form.is_none() {
            return self.err_kind(
                line,
                crate::error::ErrorKind::Other,
                subject.clone(),
                format!("{subject}: the runner plays a desktop module's sounds in a form's handler only, so a program with no form takes the Cargo path"),
            );
        }
        let args: Vec<Expr> = using
            .iter()
            .map(|a| match a {
                CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
            })
            .collect();
        let (sent, answers): (&[Expr], Vec<(&str, Option<&Expr>)>) = match name {
            "AUDIO-PLAY" | "AUDIO-STATE" if args.is_empty() => {
                let what = if name == "AUDIO-PLAY" { "the file" } else { "the data item that receives the state" };
                return self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject} takes {what}"));
            }
            "AUDIO-PLAY" => (&args[..1], vec![("AudioStatus", args.get(1))]),
            "AUDIO-STATE" => (&[], vec![("AudioState", args.first()), ("AudioPosition", args.get(1)), ("AudioDuration", args.get(2))]),
            _ => (&[], vec![("AudioStatus", args.first())]),
        };
        let seg = Seg { name, args: sent, parens: true, index: None };
        self.build_path(CTRL_APP, std::slice::from_ref(&seg))?;
        self.call_rt("rt_cmd_call");
        for (prop, dest) in answers {
            if let Some(dest) = dest {
                self.app_property_into(prop, dest, line)?;
            }
        }
        Ok(())
    }

    /// The keys of an AWS connection typed in at run time (DESKTOP-WASM-001 T48, ABI A38), with the
    /// interpreter's arguments: `COBOL::"AWS-SET-CREDENTIALS" ( connection key-id secret token
    /// [status] )`, `"AWS-CLEAR-CREDENTIALS" ( connection [status] )` and `"AWS-CREDENTIALS-SET" (
    /// connection flag )`. The runner answers at once; `status` gets the application's `AwsStatus`
    /// (`OK`, or why nothing was stored), `flag` its `AwsFlag` (`Y` or `N`). Fewer arguments than the
    /// interpreter needs are refused, so the Build takes the Cargo path, which reports them as the
    /// interpreter does. Arguments after those it reads are ignored, as it ignores them.
    fn desktop_aws(&mut self, name: &str, using: &[CallArg], line: u32) -> GenResult<()> {
        let subject = format!("COBOL::\"{name}\"");
        if self.form.is_none() {
            return self.err_kind(
                line,
                crate::error::ErrorKind::Other,
                subject.clone(),
                format!("{subject}: the runner serves AWS in a form's handler only, so a program with no form takes the Cargo path"),
            );
        }
        let args: Vec<Expr> = using
            .iter()
            .map(|a| match a {
                CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
            })
            .collect();
        let (needed, what) = match name {
            "AWS-SET-CREDENTIALS" => (4, "the connection, the access key id, the secret access key and the session token"),
            "AWS-CREDENTIALS-SET" => (2, "the connection and the data item that receives Y or N"),
            _ => (1, "the connection"),
        };
        if args.len() < needed {
            return self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject} takes {what}"));
        }
        let (sent, answer): (&[Expr], (&str, Option<&Expr>)) = match name {
            "AWS-SET-CREDENTIALS" => (&args[..4], ("AwsStatus", args.get(4))),
            "AWS-CREDENTIALS-SET" => (&args[..1], ("AwsFlag", args.get(1))),
            _ => (&args[..1], ("AwsStatus", args.get(1))),
        };
        let seg = Seg { name, args: sent, parens: true, index: None };
        self.build_path(CTRL_APP, std::slice::from_ref(&seg))?;
        self.call_rt("rt_cmd_call");
        if let (prop, Some(dest)) = answer {
            self.app_property_into(prop, dest, line)?;
        }
        Ok(())
    }

    /// A built-in the runner answers on the application object, in a desktop module (DESKTOP-WASM-001 T48):
    /// - the application's model list, its keys and the model providers (ABI A53): `COBOL::"MODEL-SET"`
    ///   and the other built-ins of `codes::MODEL_VERBS`, answered with the interpreter's own rules
    ///   (`cobolt_runtime::model_door`); the module requires `models`;
    /// - `COBOL::"MCP-SEARCH"` (ABI A54), answered with the interpreter's own function
    ///   (`cobolt_runtime::mcp_tool::search_text`) on the files the form's AgentObjects allowed; the module
    ///   requires `mcp.search`.
    ///
    /// The call goes to the application object with every argument as text (`cap` names what the module
    /// requires); what the built-in writes (`writes`, 0-based) comes back as the application's
    /// `BuiltinAnswer<n>`, `n` the argument's place, and is moved into that argument. A call with fewer
    /// arguments than the interpreter runs the built-in with (`min`) is refused, so the Build takes the Cargo
    /// path, which reports it as the interpreter does; so is one in a program with no form (`form_only` says
    /// why), as AWS's keys are.
    #[allow(clippy::too_many_arguments)]
    fn desktop_app_builtin(
        &mut self,
        name: &str,
        using: &[CallArg],
        line: u32,
        cap: &'static str,
        min: usize,
        writes: &[usize],
        form_only: &str,
    ) -> GenResult<()> {
        let subject = format!("COBOL::\"{name}\"");
        if self.form.is_none() {
            return self.err_kind(
                line,
                crate::error::ErrorKind::Other,
                subject.clone(),
                format!("{subject}: {form_only}, so a program with no form takes the Cargo path"),
            );
        }
        if using.len() < min {
            return self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject} takes at least {min} arguments"));
        }
        let args: Vec<Expr> = using
            .iter()
            .map(|a| match a {
                CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
            })
            .collect();
        self.names.file_caps.insert(cap);
        let seg = Seg { name, args: &args, parens: true, index: None };
        self.build_path(CTRL_APP, std::slice::from_ref(&seg))?;
        self.call_rt("rt_cmd_call");
        for &i in writes.iter().filter(|&&i| i < args.len()) {
            self.app_property_into(&format!("BuiltinAnswer{}", i + 1), &args[i], line)?;
        }
        Ok(())
    }

    /// `COBOL::"APPEND-FILE" ( path text [status] )` and `"WRITE-FILE"` in a desktop module (DESKTOP-WASM-001
    /// T39c, ABI A39): the runner writes the line as the interpreter does, in a program with or without a
    /// form. Fewer than two arguments are refused, so the Build takes the Cargo path, which reports them as
    /// the interpreter does; arguments after the status are ignored, as it ignores them.
    fn desktop_text_write(&mut self, name: &str, using: &[CallArg], line: u32) -> GenResult<()> {
        let subject = format!("COBOL::\"{name}\"");
        if using.len() < 2 {
            return self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject} takes the file's path and the text"));
        }
        let ex = |a: &CallArg| match a {
            CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
        };
        let path = self.operand_or_temp(&ex(&using[0]))?;
        let text = self.operand_or_temp(&ex(&using[1]))?;
        let status = match using.get(2) {
            Some(a) => self.receiver(&ex(a))?,
            None => 0,
        };
        self.base();
        self.i32c(i32::from(name == "APPEND-FILE"));
        self.i32c(path as i32);
        self.i32c(text as i32);
        self.i32c(status as i32);
        self.call_rt("rt_text_write");
        Ok(())
    }

    /// `COBOL::"FILE-STATUS" ( "file" item )` in a desktop module (DESKTOP-WASM-001 T39c): the last status
    /// of the file the program `SELECT`s under that name, `00` before any verb and for a name that is no
    /// such file, as the interpreter's `last_file_status` answers. The IndexedFile control's generated
    /// paragraphs call it after every verb. The file is named by a literal there; a name held in a data
    /// item is read at run time (T45c, [`Self::file_status_named`]). Fewer than two arguments are
    /// refused, so the Build takes the Cargo path.
    fn desktop_file_status(&mut self, using: &[CallArg], line: u32) -> GenResult<()> {
        let subject = "COBOL::\"FILE-STATUS\"".to_string();
        if using.len() < 2 {
            return self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject} takes the file's name and the data item that receives its status"));
        }
        let ex = |a: &CallArg| match a {
            CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
        };
        let name = ex(&using[0]);
        let Expr::Literal(Literal::String(file), _) = &name else {
            return self.file_status_named(&name, &ex(&using[1]), line);
        };
        // The interpreter keeps one status per name for the whole run, whichever program's verb set
        // it, and answers `00` for a name no verb has set. So a file of this program (or a GLOBAL one
        // around it) is read where its verbs leave its status, and so is a file other programs of the
        // module declare: the module keeps one file per name, as the interpreter does, so every program
        // that declares the name shares one handle place, and the status there is the last verb's on
        // any of them. A SORT work file answers `00`: `SORT`, `RELEASE` and `RETURN` set no status
        // under its name. A name no program declares answers `00`. A file the runner refuses is
        // refused here too.
        let handle = self.status_place(file, line)?;
        let target = self.receiver(&ex(&using[1]))?;
        self.base();
        self.i32c(handle as i32);
        self.i32c(target as i32);
        self.call_rt("rt_nf_last_status");
        Ok(())
    }

    /// Where the status `COBOL::"FILE-STATUS"` answers for the file `name` is kept: the handle place
    /// of the native file the name designates from this program, else of another program's native
    /// file of that name, else `u32::MAX` (the answer is `00`). A file the runner refuses is refused.
    fn status_place(&self, name: &str, line: u32) -> GenResult<u32> {
        let up = name.trim().to_ascii_uppercase();
        Ok(match self.file(name.trim(), line) {
            Ok(fi) if fi.kind == crate::lower::FileKind::Native => fi.handle_off,
            Ok(_) => u32::MAX,
            Err(e) if self.native_refusals.contains_key(&up) => return Err(e),
            Err(_) => self
                .file_tables
                .iter()
                .filter_map(|t| t.get(&up))
                .find(|(fi, _)| fi.kind == crate::lower::FileKind::Native)
                .map_or(u32::MAX, |(fi, _)| fi.handle_off),
        })
    }

    /// `COBOL::"FILE-STATUS" ( name-item target )` (DESKTOP-WASM-001 T45c): the file is named at run
    /// time. Every file name of the module goes into a table, each with the status place a literal of
    /// that name would read ([`Self::status_place`]); the routine reads the item's characters as the
    /// interpreter does (trimmed, in capitals) and looks them up, and a name not in the table answers
    /// `00`. The name is alphanumeric text: an item, a group, an element, a part of one.
    fn file_status_named(&mut self, name: &Expr, target: &Expr, line: u32) -> GenResult<()> {
        let subject = "COBOL::\"FILE-STATUS\"".to_string();
        let text = match name {
            Expr::RefMod { .. } => true,
            Expr::Identifier(..) | Expr::Qualified { .. } => self.resolve_item(name).is_ok_and(|i| self.is_text_item(i)),
            Expr::Subscript { base, .. } => self.resolve_item(base).is_ok_and(|i| self.is_text_item(i)),
            _ => false,
        };
        if !text {
            return self.err_kind(
                line,
                crate::error::ErrorKind::Other,
                subject.clone(),
                format!("{subject} in a WebAssembly module names the file with an alphanumeric literal or an alphanumeric or group item; the interpreter's reading of any other value as a file's name is not computed in a module, so the Build takes the Cargo path"),
            );
        }
        let names: std::collections::BTreeSet<String> =
            self.files.keys().cloned().chain(self.file_tables.iter().flat_map(|t| t.keys().cloned())).collect();
        let mut entries: Vec<(String, u32)> = Vec::new();
        for n in names {
            let place = self.status_place(&n, line)?;
            if place != u32::MAX {
                entries.push((n, place));
            }
        }
        let mut table = (entries.len() as u32).to_le_bytes().to_vec();
        for (n, place) in &entries {
            let at = self.pool.intern(n.as_bytes());
            table.extend_from_slice(&at.to_le_bytes());
            table.extend_from_slice(&(n.len() as u32).to_le_bytes());
            table.extend_from_slice(&place.to_le_bytes());
        }
        let table = self.pool.intern(&table);
        let name = self.operand_or_temp(name)?;
        let target = self.receiver(target)?;
        self.base();
        self.i32c(name as i32);
        self.i32c(table as i32);
        self.i32c(target as i32);
        self.call_rt("rt_nf_last_status_named");
        Ok(())
    }

    /// `COBOL::"IMAGE-FIT" ( source target width height status )` and `"IMAGE-INFO" ( path width height
    /// format status )` in a desktop module (DESKTOP-WASM-001 T39c, ABI A39): the runner reads and fits the
    /// picture with the interpreter's own code, in a program with or without a form. Fewer than five
    /// arguments are refused, so the Build takes the Cargo path, which reports them as the interpreter does.
    fn desktop_image(&mut self, name: &str, using: &[CallArg], line: u32) -> GenResult<()> {
        let subject = format!("COBOL::\"{name}\"");
        if using.len() < 5 {
            let what = if name == "IMAGE-FIT" {
                "the source, the target, the width, the height and the data item that receives the status"
            } else {
                "the path and the data items that receive the width, the height, the format and the status"
            };
            return self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject} takes {what}"));
        }
        let ex = |a: &CallArg| match a {
            CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
        };
        let mut opds = [0u32; 5];
        let sent = if name == "IMAGE-FIT" { 4 } else { 1 };
        for (i, a) in using.iter().take(5).enumerate() {
            opds[i] = if i < sent { self.operand_or_temp(&ex(a))? } else { self.receiver(&ex(a))? };
        }
        self.base();
        for o in opds {
            self.i32c(o as i32);
        }
        self.call_rt(if name == "IMAGE-FIT" { "rt_image_fit" } else { "rt_image_info" });
        Ok(())
    }

    /// `COBOL::"OPEN-DB" ( connection handle status )`, `"EXEC-SQL" ( handle query count status )`,
    /// `"FETCH-ROW" ( handle column value status )`, `"NEXT-ROW" ( handle flag )`, `"ROW-COUNT" ( handle
    /// count )` and `"CLOSE-DB" ( handle )` in a desktop module (DESKTOP-WASM-001 T41b, ABI A42): the
    /// runner keeps the connections, one set per form instance as each form's interpreter keeps its own,
    /// and runs each call with the interpreter's own code. Fewer arguments than the built-in takes are
    /// refused, so the Build takes the Cargo path, where the interpreter does what it does with them;
    /// arguments after them are ignored, as it ignores them.
    fn desktop_sql_db(&mut self, name: &str, using: &[CallArg], line: u32) -> GenResult<()> {
        use cobolt_wasm_rt::sql::db;
        let subject = format!("COBOL::\"{name}\"");
        // The operation, the arguments it takes, how many of them the program sends (the rest are receivers).
        let (op, takes, sent, what) = match name {
            "OPEN-DB" => (db::OPEN, 3, 1, "the connection string and the data items that receive the handle and the status"),
            "EXEC-SQL" => (db::EXEC, 4, 2, "the handle, the statement and the data items that receive the count and the status"),
            "FETCH-ROW" => (db::FETCH_ROW, 4, 2, "the handle, the column and the data items that receive the value and the status"),
            "NEXT-ROW" => (db::NEXT_ROW, 2, 1, "the handle and the data item that receives the flag"),
            "ROW-COUNT" => (db::ROW_COUNT, 2, 1, "the handle and the data item that receives the count"),
            _ => (db::CLOSE, 1, 1, "the handle"),
        };
        if using.len() < takes {
            return self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject} takes {what}"));
        }
        let ex = |a: &CallArg| match a {
            CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
        };
        let mut opds = [0u32; 4];
        for (i, a) in using.iter().take(takes).enumerate() {
            opds[i] = if i < sent { self.operand_or_temp(&ex(a))? } else { self.receiver(&ex(a))? };
        }
        self.base();
        self.i32c(op as i32);
        for o in opds {
            self.i32c(o as i32);
        }
        self.call_rt("rt_sql_db");
        Ok(())
    }

    /// `COBOL::"HTTP-GET" ( url response http-status )`, `"HTTP-POST"` and `"HTTP-PUT" ( url body response
    /// http-status )`, `"HTTP-DELETE" ( url response http-status )`, `"HTTP-SET-HEADER" ( name value )`,
    /// `"HTTP-CLEAR-HEADERS"` and `"HTTP-DOWNLOAD" ( url file http-status status )` in a desktop module
    /// (DESKTOP-WASM-001 T42, ABI A45): the runner keeps one HTTP client per form instance, as each form's
    /// interpreter keeps its own, and runs each call with the interpreter's own client. Fewer arguments
    /// than the built-in takes are refused, so the Build takes the Cargo path, where the interpreter does
    /// what it does with them; arguments after them are ignored, as it ignores them.
    fn desktop_http(&mut self, name: &str, using: &[CallArg], line: u32) -> GenResult<()> {
        use cobolt_wasm_rt::deskio::http;
        let subject = format!("COBOL::\"{name}\"");
        // The operation, the arguments it takes, and the place of each: the two sent (`a`, `b`) and the
        // receivers of the answer's text and number; `None` for a place the built-in has not.
        type Places = [Option<usize>; 4];
        let (op, takes, places, what): (u32, usize, Places, &str) = match name {
            "HTTP-GET" => (http::GET, 3, [Some(0), None, Some(1), Some(2)], "the URL and the data items that receive the response and the status code"),
            "HTTP-POST" => (http::POST, 4, [Some(0), Some(1), Some(2), Some(3)], "the URL, the body and the data items that receive the response and the status code"),
            "HTTP-PUT" => (http::PUT, 4, [Some(0), Some(1), Some(2), Some(3)], "the URL, the body and the data items that receive the response and the status code"),
            "HTTP-DELETE" => (http::DELETE, 3, [Some(0), None, Some(1), Some(2)], "the URL and the data items that receive the response and the status code"),
            "HTTP-SET-HEADER" => (http::SET_HEADER, 2, [Some(0), Some(1), None, None], "the header's name and its value"),
            "HTTP-CLEAR-HEADERS" => (http::CLEAR_HEADERS, 0, [None; 4], ""),
            _ => (http::DOWNLOAD, 4, [Some(0), Some(1), Some(3), Some(2)], "the URL, the file and the data items that receive the status code and the status"),
        };
        if using.len() < takes {
            return self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject} takes {what}"));
        }
        let ex = |a: &CallArg| match a {
            CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
        };
        let mut opds = [0u32; 4];
        for (slot, place) in places.iter().enumerate() {
            if let Some(i) = place {
                let e = ex(&using[*i]);
                opds[slot] = if slot < 2 { self.operand_or_temp(&e)? } else { self.receiver(&e)? };
            }
        }
        self.base();
        self.i32c(op as i32);
        for o in opds {
            self.i32c(o as i32);
        }
        self.call_rt("rt_http");
        Ok(())
    }

    /// `COBOL::"PARTIQL" ( file statement count status [arguments] )`, `"PARTIQL-FETCH" ( file row
    /// status )` and `"PARTIQL-ERROR" ( file text )` in a desktop module (DESKTOP-WASM-001 T48, ABI
    /// A56): the runner runs the statement with the interpreter's own rules on the file the program's
    /// `SELECT` declares and holds open, and keeps each file's results for the program. The module
    /// sends the program's files (their numbers and slots), so the runner finds the one the file
    /// argument names among them. Each `?` argument goes with its kind, as the interpreter types it by
    /// its declaration (`partiql_kind`; a control's property is a text here, as the interpreter's
    /// value of it is). Fewer arguments than the built-in takes are refused, so the Build takes the
    /// Cargo path, where the interpreter does what it does with them.
    fn desktop_partiql(&mut self, name: &str, using: &[CallArg], line: u32) -> GenResult<()> {
        use cobolt_wasm_rt::pq;
        /// More `?` arguments than the request has room for in the scratch area.
        const MOST_ARGS: usize = 256;
        let subject = format!("COBOL::\"{name}\"");
        let (op, takes, what) = match name {
            "PARTIQL" => (pq::RUN, 4, "the file, the statement and the data items that receive the count and the status, then the statement's arguments"),
            "PARTIQL-FETCH" => (pq::FETCH, 3, "the file and the data items that receive the row and the status"),
            _ => (pq::ERROR, 2, "the file and the data item that receives the error's text"),
        };
        if using.len() < takes {
            return self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject} takes {what}"));
        }
        if using.len() - takes > MOST_ARGS {
            return self.err_kind(line, crate::error::ErrorKind::Other, subject.clone(), format!("{subject} with more than {MOST_ARGS} arguments is not on the WebAssembly path"));
        }
        let ex = |a: &CallArg| match a {
            CallArg::ByReference(e) | CallArg::ByContent(e) | CallArg::ByValue(e) => e.clone(),
        };
        let file = self.operand_or_temp(&ex(&using[0]))?;
        let mut words = vec![op, file, 0, 0, 0, 0, 0];
        match op {
            pq::RUN => {
                words[2] = self.operand_or_temp(&ex(&using[1]))?;
                words[3] = self.receiver(&ex(&using[2]))?;
                words[4] = self.receiver(&ex(&using[3]))?;
            }
            pq::FETCH => {
                words[2] = self.receiver(&ex(&using[1]))?;
                words[3] = self.receiver(&ex(&using[2]))?;
            }
            _ => words[2] = self.receiver(&ex(&using[1]))?,
        }
        // The program's files, in the order of their numbers.
        let mut files: Vec<(u32, u32)> = self.native_ids.values().copied().collect();
        files.sort_unstable();
        let mut table = vec![files.len() as u32];
        for (id, handle_off) in files {
            table.extend([id, handle_off]);
        }
        words[5] = self.pool.spec(&table);
        if op == pq::RUN {
            let args = &using[takes..];
            words[6] = args.len() as u32;
            for a in args {
                let e = ex(a);
                let o = self.operand_or_temp(&e)?;
                let kind = match self.partiql_kind(&e, o) {
                    'N' => pq::NUMBER,
                    _ => pq::TEXT,
                };
                words.extend([o, u32::from(kind)]);
            }
        }
        // The import `pq_file` makes the module require `partiql.files` (A56).
        let spec = self.pool.spec(&words);
        self.base();
        self.i32c(spec as i32);
        self.call_rt("rt_partiql");
        Ok(())
    }

    /// Move the application object's property `prop` (as text) into `dest`.
    fn app_property_into(&mut self, prop: &str, dest: &Expr, line: u32) -> GenResult<()> {
        let k = self.ctemp_next;
        if k >= cobolt_wasm_rt::state::NCTEMP as u32 {
            return self.too_many_reads(line, "this statement reads more than four properties or character-valued functions; split it into statements that read at most four");
        }
        self.ctemp_next += 1;
        let value = Seg { name: prop, args: &[], parens: false, index: None };
        self.build_path(CTRL_APP, std::slice::from_ref(&value))?;
        self.i32c(Q_GET as i32);
        self.i32c(k as i32);
        self.i32c(1);
        self.call_rt("rt_cmd_query");
        let src = self.pool.opd_ctemp(k);
        self.move_operand_to(src, dest, line)
    }
}

/// What a layer answers, as the end of a refusal: `Name (read only), Visible, …`.
fn layer_props_text() -> String {
    cobolt_forms::model::LAYER_PROPS
        .iter()
        .map(|p| if p.eq_ignore_ascii_case("Name") { format!("{p} (read only)") } else { (*p).to_string() })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The routine's code for a value method, by any of its spellings.
fn value_method_code(name: &str) -> Option<u32> {
    use cobolt_wasm_rt::strings::*;
    let canonical = cobolt_ast::methods::canonical_data_item_method(name).unwrap_or(name);
    Some(match canonical.to_ascii_uppercase().as_str() {
        "TRIM" => VM_TRIM,
        "UPPER" => VM_UPPER,
        "LOWER" => VM_LOWER,
        "REPLACE" => VM_REPLACE,
        "LEN" => VM_LEN,
        "SPLIT" => VM_SPLIT,
        _ => return None,
    })
}

#[allow(dead_code)]
fn _unused(_: I<'static>) {}

#[cfg(test)]
mod tests {
    /// The runtime's copy of the operations that cross `http_call` (A45) is the ABI's: the runtime cannot
    /// link the ABI crate, so this crate, which links both, holds the two equal.
    #[test]
    fn the_runtime_speaks_the_abis_http_operations() {
        use cobolt_wasm_abi::http as abi;
        use cobolt_wasm_rt::deskio::http as rt;
        let ops = [
            ("GET", abi::GET, rt::GET),
            ("POST", abi::POST, rt::POST),
            ("PUT", abi::PUT, rt::PUT),
            ("DELETE", abi::DELETE, rt::DELETE),
            ("SET_HEADER", abi::SET_HEADER, rt::SET_HEADER),
            ("CLEAR_HEADERS", abi::CLEAR_HEADERS, rt::CLEAR_HEADERS),
            ("DOWNLOAD", abi::DOWNLOAD, rt::DOWNLOAD),
            ("TAIL", abi::TAIL, rt::TAIL),
        ];
        for (name, a, r) in ops {
            assert_eq!(a, r, "http::{name}: the ABI says {a}, the runtime {r}");
        }
        println!("http_call: {} of {} operations equal in the ABI and the runtime", ops.len(), ops.len());
    }

    /// The runtime's copy of what crosses `pq_file` (A56) is the ABI's, as for `http_call`.
    #[test]
    fn the_runtime_speaks_the_abis_partiql_operations() {
        use cobolt_wasm_abi::partiql as abi;
        use cobolt_wasm_rt::pq as rt;
        let words = [("RUN", abi::RUN, rt::RUN), ("FETCH", abi::FETCH, rt::FETCH), ("ERROR", abi::ERROR, rt::ERROR), ("TAIL", abi::TAIL, rt::TAIL)];
        for (name, a, r) in words {
            assert_eq!(a, r, "partiql::{name}: the ABI says {a}, the runtime {r}");
        }
        let kinds = [("NUMBER", abi::NUMBER, rt::NUMBER), ("TEXT", abi::TEXT, rt::TEXT)];
        for (name, a, r) in kinds {
            assert_eq!(a, r, "partiql::{name}: the ABI says {a}, the runtime {r}");
        }
        assert_eq!(abi::TAIL, cobolt_wasm_abi::http::TAIL, "the runner writes a TAIL answer with http::part");
        println!("pq_file: {} of {} operations and {} of {} kinds equal in the ABI and the runtime; TAIL equals http's", words.len(), words.len(), kinds.len(), kinds.len());
    }
}
