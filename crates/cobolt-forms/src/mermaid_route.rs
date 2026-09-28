// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A flowchart's connectors, drawn the way a process diagram is drawn:
//! ORTHOGONAL — every segment horizontal or vertical — and attached to a
//! shape at the middle of one of its sides, which on a decision diamond is
//! one of its vertices (operator, 2026-09-28: the lines slanted, and met the
//! boxes wherever they happened to arrive).
//!
//! The Mermaid layout still places the nodes; only each edge's points are
//! replaced before the SVG is written. For every edge a handful of candidate
//! routes is built — straight, a Z, an L, a detour round the nodes in the
//! way — and the cheapest wins: shortest, fewest bends, never through
//! another shape, and preferring a port no other edge already uses, so the
//! two ways out of a decision leave by two different vertices.

use mermaid_rs_renderer::{DiagramKind, Layout};

/// A node's box: left, top, right, bottom.
#[derive(Debug, Clone, Copy)]
struct Bx {
    l: f32,
    t: f32,
    r: f32,
    b: f32,
}

impl Bx {
    fn cx(&self) -> f32 {
        (self.l + self.r) / 2.0
    }
    fn cy(&self) -> f32 {
        (self.t + self.b) / 2.0
    }
    /// The middle of a side: N, E, S, W.
    fn port(&self, side: Side) -> (f32, f32) {
        match side {
            Side::N => (self.cx(), self.t),
            Side::S => (self.cx(), self.b),
            Side::E => (self.r, self.cy()),
            Side::W => (self.l, self.cy()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Side {
    N,
    E,
    S,
    W,
}

/// Room left between a connector and the shape it goes round.
const LANE: f32 = 18.0;
/// What one bend costs, in pixels of length.
const BEND: f32 = 40.0;
/// What sharing a port already taken costs.
const SHARED_PORT: f32 = 60.0;
/// What passing through another shape costs — enough that any clear route wins.
const THROUGH_SHAPE: f32 = 100_000.0;

/// Make every edge of a flowchart orthogonal and attach it at a side's
/// middle. Other diagram kinds are left exactly as laid out.
pub fn orthogonalize(layout: &mut Layout) {
    if layout.kind != DiagramKind::Flowchart {
        return;
    }
    let boxes: std::collections::HashMap<String, Bx> = layout
        .nodes
        .iter()
        .filter(|(_, n)| !n.hidden)
        .map(|(id, n)| (id.clone(), Bx { l: n.x, t: n.y, r: n.x + n.width, b: n.y + n.height }))
        .collect();
    let mut used: std::collections::HashMap<(String, Side), usize> = Default::default();
    // Detour lanes taken so far, per side of the drawing: each detour runs
    // in a lane of its own, never on top of another.
    let mut lanes: std::collections::HashMap<Side, usize> = Default::default();
    for edge in layout.edges.iter_mut() {
        let (Some(a), Some(b)) = (boxes.get(&edge.from), boxes.get(&edge.to)) else { continue };
        if edge.from == edge.to {
            continue; // a loop keeps its own shape
        }
        let others: Vec<Bx> = boxes
            .iter()
            .filter(|(id, _)| **id != edge.from && **id != edge.to)
            .map(|(_, b)| *b)
            .collect();
        let mut best: Option<(f32, Vec<(f32, f32)>, Side, Side, Option<Side>)> = None;
        for (route, out_side, in_side, lane) in candidates(a, b, &others, &lanes) {
            let mut cost = length(&route) + BEND * (route.len().saturating_sub(2)) as f32;
            cost += THROUGH_SHAPE * crossings(&route, a, b, &others) as f32;
            cost += SHARED_PORT * *used.get(&(edge.from.clone(), out_side)).unwrap_or(&0) as f32;
            cost += SHARED_PORT * 0.5 * *used.get(&(edge.to.clone(), in_side)).unwrap_or(&0) as f32;
            if best.as_ref().is_none_or(|(c, ..)| cost < *c) {
                best = Some((cost, route, out_side, in_side, lane));
            }
        }
        let Some((_, route, out_side, in_side, lane)) = best else { continue };
        *used.entry((edge.from.clone(), out_side)).or_default() += 1;
        *used.entry((edge.to.clone(), in_side)).or_default() += 1;
        if let Some(side) = lane {
            *lanes.entry(side).or_default() += 1;
        }
        // A label sits on a straight run of its own route, where it covers
        // no shape: the longest runs are tried first, each at its middle and
        // then a quarter from either end.
        if let Some(t) = &edge.label {
            let (hw, hh) = (t.width / 2.0 + 4.0, t.height / 2.0 + 2.0);
            let mut runs: Vec<((f32, f32), (f32, f32))> = route.windows(2).map(|w| (w[0], w[1])).collect();
            runs.sort_by(|x, y| seg_len(y.0, y.1).total_cmp(&seg_len(x.0, x.1)));
            let clear = |(x, y): (f32, f32)| {
                boxes.values().all(|b| x + hw <= b.l || x - hw >= b.r || y + hh <= b.t || y - hh >= b.b)
            };
            // Along each run from its middle outwards, every 8 px — a label
            // on a lane beside a column of shapes finds the gap between two.
            let spots = runs.iter().flat_map(|(p, q)| {
                let len = seg_len(*p, *q);
                let n = (len / 16.0) as i32;
                let (p, q) = (*p, *q);
                (0..=n).flat_map(move |k| [k, -k]).map(move |k| {
                    let f = (0.5 + k as f32 * 8.0 / len.max(1.0)).clamp(0.0, 1.0);
                    (p.0 + (q.0 - p.0) * f, p.1 + (q.1 - p.1) * f)
                })
            });
            let first = runs.first().map(|(p, q)| ((p.0 + q.0) / 2.0, (p.1 + q.1) / 2.0)).unwrap_or(route[0]);
            edge.label_anchor = Some(spots.clone().find(|s| clear(*s)).unwrap_or(first));
        }
        if edge.start_label.is_some() {
            edge.start_label_anchor = Some(route[0]);
        }
        if edge.end_label.is_some() {
            edge.end_label_anchor = route.last().copied();
        }
        edge.points = route;
    }
    fit(layout);
}

/// Grow the drawing to hold every route: a detour lane past the left or top
/// edge moves the whole drawing over, rather than being squeezed against it.
fn fit(layout: &mut Layout) {
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut take = |x0: f32, y0: f32, x1: f32, y1: f32| {
        min_x = min_x.min(x0);
        min_y = min_y.min(y0);
        max_x = max_x.max(x1);
        max_y = max_y.max(y1);
    };
    for e in &layout.edges {
        for (x, y) in &e.points {
            take(*x, *y, *x, *y);
        }
        // A label is centred on its anchor, so the whole label box has to
        // fit — a label on a lane at the edge was cut in half.
        for (label, anchor) in [(&e.label, e.label_anchor), (&e.start_label, e.start_label_anchor), (&e.end_label, e.end_label_anchor)] {
            if let (Some(t), Some((x, y))) = (label, anchor) {
                let (hw, hh) = (t.width / 2.0 + 6.0, t.height / 2.0 + 4.0);
                take(x - hw, y - hh, x + hw, y + hh);
            }
        }
    }
    if min_x == f32::MAX {
        return;
    }
    let margin = 8.0;
    let dx = (margin - min_x).max(0.0);
    let dy = (margin - min_y).max(0.0);
    if dx > 0.0 || dy > 0.0 {
        for n in layout.nodes.values_mut() {
            n.x += dx;
            n.y += dy;
        }
        for sg in layout.subgraphs.iter_mut() {
            sg.x += dx;
            sg.y += dy;
        }
        let shift = |p: &mut (f32, f32)| {
            p.0 += dx;
            p.1 += dy;
        };
        for e in layout.edges.iter_mut() {
            e.points.iter_mut().for_each(shift);
            e.label_anchor.as_mut().map(shift);
            e.start_label_anchor.as_mut().map(shift);
            e.end_label_anchor.as_mut().map(shift);
        }
    }
    layout.width = (layout.width + dx).max(max_x + dx + margin);
    layout.height = (layout.height + dy).max(max_y + dy + margin);
}

fn seg_len(p: (f32, f32), q: (f32, f32)) -> f32 {
    (p.0 - q.0).abs() + (p.1 - q.1).abs()
}

fn length(route: &[(f32, f32)]) -> f32 {
    route.windows(2).map(|w| seg_len(w[0], w[1])).sum()
}

/// Every route worth trying from `a` to `b`, each with the sides it leaves
/// and arrives by. All are orthogonal and start and end at a side's middle.
/// The last item names the drawing side a detour's lane runs on.
#[allow(clippy::type_complexity)]
fn candidates(
    a: &Bx,
    b: &Bx,
    others: &[Bx],
    lanes: &std::collections::HashMap<Side, usize>,
) -> Vec<(Vec<(f32, f32)>, Side, Side, Option<Side>)> {
    use Side::*;
    let mut out = Vec::new();
    let mut add_lane = |route: Vec<(f32, f32)>, s: Side, t: Side, lane: Option<Side>| {
        let route = simplify(route);
        if orthogonal(&route) {
            out.push((route, s, t, lane));
        }
    };
    let mut add = |route: Vec<(f32, f32)>, s: Side, t: Side| add_lane(route, s, t, None);
    // Straight and Z, vertically: out of the bottom into the top, or the
    // reverse for a node above.
    for (s, t) in [(S, N), (N, S)] {
        let p = a.port(s);
        let q = b.port(t);
        let ahead = if s == S { q.1 > p.1 } else { q.1 < p.1 };
        if ahead {
            let ym = (p.1 + q.1) / 2.0;
            add(vec![p, (p.0, ym), (q.0, ym), q], s, t);
        }
    }
    // Straight and Z, horizontally.
    for (s, t) in [(E, W), (W, E)] {
        let p = a.port(s);
        let q = b.port(t);
        let ahead = if s == E { q.0 > p.0 } else { q.0 < p.0 };
        if ahead {
            let xm = (p.0 + q.0) / 2.0;
            add(vec![p, (xm, p.1), (xm, q.1), q], s, t);
        }
    }
    // L: out sideways, then down or up into the top or bottom; or out of the
    // bottom or top, then sideways into a side.
    for s in [E, W] {
        for t in [N, S] {
            let p = a.port(s);
            let q = b.port(t);
            add(vec![p, (q.0, p.1), q], s, t);
        }
    }
    for s in [N, S] {
        for t in [E, W] {
            let p = a.port(s);
            let q = b.port(t);
            add(vec![p, (p.0, q.1), q], s, t);
        }
    }
    // Detours: out of one side, along a lane clear of every shape between the
    // two, and in by the same side.
    let (top, bottom) = (a.t.min(b.t), a.b.max(b.b));
    let (left, right) = (a.l.min(b.l), a.r.max(b.r));
    let band_v: Vec<&Bx> = others.iter().filter(|o| o.b > top && o.t < bottom).collect();
    let band_h: Vec<&Bx> = others.iter().filter(|o| o.r > left && o.l < right).collect();
    let step = |side: Side| LANE * (1 + lanes.get(&side).copied().unwrap_or(0)) as f32;
    let xl = band_v.iter().map(|o| o.l).fold(left, f32::min) - step(W);
    let xr = band_v.iter().map(|o| o.r).fold(right, f32::max) + step(E);
    let yt = band_h.iter().map(|o| o.t).fold(top, f32::min) - step(N);
    let yb = band_h.iter().map(|o| o.b).fold(bottom, f32::max) + step(S);
    drop(add);
    let (pw, qw) = (a.port(W), b.port(W));
    add_lane(vec![pw, (xl, pw.1), (xl, qw.1), qw], W, W, Some(W));
    let (pe, qe) = (a.port(E), b.port(E));
    add_lane(vec![pe, (xr, pe.1), (xr, qe.1), qe], E, E, Some(E));
    let (pn, qn) = (a.port(N), b.port(N));
    add_lane(vec![pn, (pn.0, yt), (qn.0, yt), qn], N, N, Some(N));
    let (ps, qs) = (a.port(S), b.port(S));
    add_lane(vec![ps, (ps.0, yb), (qs.0, yb), qs], S, S, Some(S));
    out
}

/// Drop repeated points and merge runs in one direction.
fn simplify(route: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    let mut out: Vec<(f32, f32)> = Vec::with_capacity(route.len());
    for p in route {
        if out.last().is_some_and(|q| (q.0 - p.0).abs() < 0.01 && (q.1 - p.1).abs() < 0.01) {
            continue;
        }
        if out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            let collinear = ((a.0 - b.0).abs() < 0.01 && (b.0 - p.0).abs() < 0.01)
                || ((a.1 - b.1).abs() < 0.01 && (b.1 - p.1).abs() < 0.01);
            if collinear {
                out.pop();
            }
        }
        out.push(p);
    }
    out
}

fn orthogonal(route: &[(f32, f32)]) -> bool {
    route.len() >= 2 && route.windows(2).all(|w| (w[0].0 - w[1].0).abs() < 0.01 || (w[0].1 - w[1].1).abs() < 0.01)
}

/// How many times the route passes through a shape: any other node, and its
/// own two ends except where it leaves and arrives.
fn crossings(route: &[(f32, f32)], a: &Bx, b: &Bx, others: &[Bx]) -> usize {
    let hits = |bx: &Bx, w: &[(f32, f32)]| -> bool {
        let inset = 1.0;
        let (p, q) = (w[0], w[1]);
        let (x0, x1) = (p.0.min(q.0), p.0.max(q.0));
        let (y0, y1) = (p.1.min(q.1), p.1.max(q.1));
        x1 > bx.l + inset && x0 < bx.r - inset && y1 > bx.t + inset && y0 < bx.b - inset
    };
    let mut n = 0;
    for w in route.windows(2) {
        n += others.iter().filter(|o| hits(o, w)).count();
        if hits(a, w) {
            n += 1;
        }
        if hits(b, w) {
            n += 1;
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(src: &str) -> Layout {
        let parsed = mermaid_rs_renderer::parse_mermaid(src).expect("parses");
        let opts = mermaid_rs_renderer::RenderOptions::default();
        mermaid_rs_renderer::compute_layout(&parsed.graph, &opts.theme, &opts.layout)
    }

    /// The operator's process (2026-09-28): a decision with two ways out, one
    /// of which skips a step. Every connector is orthogonal, starts and ends
    /// at the middle of a side of its own two shapes (a diamond's vertex),
    /// crosses no other shape, and the decision's two ways out leave by two
    /// different vertices.
    #[test]
    fn flowchart_connectors_are_orthogonal_and_meet_a_side_at_its_middle() {
        let mut l = layout(
            "flowchart TD\n A[Elaboração] --> B{Revisão Legal/LGPD?}\n B -->|Necessário| C[Ajuste de Cláusulas]\n \
             B -->|OK| D[Aprovação de Alçada]\n C --> D\n D --> E[Assinatura Final]\n E --> F[Arquivo]\n F -.->|Revisão| A",
        );
        orthogonalize(&mut l);
        let boxes: std::collections::HashMap<&str, Bx> = l
            .nodes
            .iter()
            .map(|(id, n)| (id.as_str(), Bx { l: n.x, t: n.y, r: n.x + n.width, b: n.y + n.height }))
            .collect();
        let near = |p: (f32, f32), q: (f32, f32)| (p.0 - q.0).abs() < 0.6 && (p.1 - q.1).abs() < 0.6;
        let is_port = |bx: &Bx, p: (f32, f32)| [Side::N, Side::E, Side::S, Side::W].iter().any(|s| near(bx.port(*s), p));
        let mut decision_out = Vec::new();
        println!("\n  ── orthogonal connectors ──");
        for e in &l.edges {
            let (a, b) = (&boxes[e.from.as_str()], &boxes[e.to.as_str()]);
            println!("  {} -> {}: {:?}", e.from, e.to, e.points.iter().map(|p| (p.0.round(), p.1.round())).collect::<Vec<_>>());
            assert!(orthogonal(&e.points), "{} -> {} is orthogonal", e.from, e.to);
            assert!(is_port(a, e.points[0]), "{} -> {} leaves by a side's middle", e.from, e.to);
            assert!(is_port(b, *e.points.last().unwrap()), "{} -> {} arrives at a side's middle", e.from, e.to);
            let others: Vec<Bx> = boxes.iter().filter(|(id, _)| **id != e.from && **id != e.to).map(|(_, b)| *b).collect();
            assert_eq!(crossings(&e.points, a, b, &others), 0, "{} -> {} passes through no shape", e.from, e.to);
            if e.from == "B" {
                decision_out.push(e.points[0]);
            }
        }
        // Two vertical runs on one x never share any stretch of y.
        let mut runs: Vec<(f32, f32, f32, String)> = Vec::new();
        for e in &l.edges {
            for w in e.points.windows(2) {
                if (w[0].0 - w[1].0).abs() < 0.01 && (w[0].1 - w[1].1).abs() > 0.01 {
                    runs.push((w[0].0, w[0].1.min(w[1].1), w[0].1.max(w[1].1), format!("{}->{}", e.from, e.to)));
                }
            }
        }
        for (i, r) in runs.iter().enumerate() {
            for q in &runs[i + 1..] {
                let shared = (r.0 - q.0).abs() < 0.5 && r.1 < q.2 - 0.5 && q.1 < r.2 - 0.5;
                assert!(!shared || r.3 == q.3, "{} and {} run on top of each other", r.3, q.3);
            }
        }
        assert!(l.edges.iter().flat_map(|e| e.points.iter()).all(|p| p.0 >= 7.9 && p.1 >= 7.9), "inside the drawing");
        assert_eq!(decision_out.len(), 2);
        assert!(!near(decision_out[0], decision_out[1]), "the decision's two ways out leave by two vertices");
        // No label covers a shape.
        for e in &l.edges {
            if let (Some(t), Some((x, y))) = (&e.label, e.label_anchor) {
                let (hw, hh) = (t.width / 2.0, t.height / 2.0);
                for (id, b) in &boxes {
                    let over = x + hw > b.l && x - hw < b.r && y + hh > b.t && y - hh < b.b;
                    assert!(!over, "the label of {} -> {} covers {id}", e.from, e.to);
                }
            }
        }
    }
}
