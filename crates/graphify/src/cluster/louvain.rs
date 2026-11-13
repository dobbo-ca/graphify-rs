//! Port of `louvain.go`: gonum `community.Modularize` on an unweighted graph, seed 42.

use gostd::pcg::{PcgSource, Rand};

use super::cluster::SEED;

/// Go `deltaQTol` (louvain.go:10): the smallest modularity gain that still counts as a move.
const DELTA_Q_TOL: f64 = 1e-15;

// Go `lookAtAll` (louvain.go:13) is a package variable; here it is the `look_at_all` argument of
// `modularize_with` and `Level::local_move` (a `static` would race between parallel tests).

/// Go `level` (louvain.go:17): one step of the Louvain hierarchy, a weighted graph whose nodes are
/// the communities of the level below. Row `u` of the adjacency is `nbr[off[u]..off[u + 1]]`:
/// distinct neighbours, never the node itself.
struct Level {
    off: Vec<usize>,
    nbr: Vec<i32>,
    /// Edge weights, parallel to `nbr`; `None` means every weight is 1 (Go: a nil slice).
    w: Option<Vec<f64>>,
    /// Go field `self` (a keyword in Rust): weight internal to each node.
    self_w: Vec<f64>,
    /// Current communities, as node indices.
    comms: Vec<Vec<usize>>,
    parent: Option<Box<Level>>,
}

/// Go `modularize` (louvain.go:37): Louvain over a deduplicated, self-loop-free undirected
/// adjacency on nodes `0..n`, as node-index groups. Reproduces gonum v0.15.1
/// `community.Modularize(g, 1, rand.NewSource(seed))` on an unweighted graph exactly.
pub(super) fn modularize(adj: &[Vec<i64>]) -> Vec<Vec<i64>> {
    modularize_with(adj, false)
}

/// Go `modularize` with the package variable `lookAtAll` as an argument: `true` turns the skip in
/// `localMove` off.
pub(super) fn modularize_with(adj: &[Vec<i64>], look_at_all: bool) -> Vec<Vec<i64>> {
    let n = adj.len();
    let mut l = Level {
        off: vec![0; n + 1],
        nbr: Vec::new(),
        w: None,
        self_w: vec![0.0; n],
        comms: vec![Vec::new(); n],
        parent: None,
    };
    for (i, row) in adj.iter().enumerate() {
        for &v in row {
            l.nbr.push(v as i32);
        }
        l.off[i + 1] = l.nbr.len();
        l.comms[i] = vec![i];
    }
    let mut r = Rand::new(PcgSource::new(SEED));
    let mut rnd = |n: usize| r.intn(n as i64) as usize;
    while l.local_move(&mut rnd, look_at_all) {
        l = l.reduce();
    }
    l.expand()
        .into_iter()
        .map(|g| g.into_iter().map(|m| m as i64).collect())
        .collect()
}

/// The visit order of the first sweep of `localMove` (louvain.go:130-133) on `n` nodes: `order`
/// after the Fisher-Yates driven by a fresh seed-42 PCG (`rand.New(rand.NewSource(seed)).Intn`).
pub(super) fn local_move_order(n: usize) -> Vec<usize> {
    let mut r = Rand::new(PcgSource::new(SEED));
    let mut rnd = |n: usize| r.intn(n as i64) as usize;
    let mut order: Vec<usize> = (0..n).collect();
    shuffle(&mut order, &mut rnd);
    order
}

/// louvain.go:130-133: Go's `for i := range order[:n-1] { j := i + rnd(n-i); swap }`.
fn shuffle(order: &mut [usize], rnd: &mut dyn FnMut(usize) -> usize) {
    let n = order.len();
    for i in 0..n - 1 {
        let j = i + rnd(n - i);
        order.swap(i, j);
    }
}

/// Go `weights` (louvain.go:216): the weights of row `u`, `None` when every weight is 1. A free
/// function over the two fields so callers can keep other fields of the level borrowed.
fn weights<'a>(w: &'a Option<Vec<f64>>, off: &[usize], u: usize) -> Option<&'a [f64]> {
    w.as_ref().map(|w| &w[off[u]..off[u + 1]])
}

impl Level {
    /// Go `(*level).expand` (louvain.go:55): maps the communities back to node indices of the
    /// bottom level.
    fn expand(&self) -> Vec<Vec<usize>> {
        let Some(parent) = &self.parent else {
            return self.comms.clone();
        };
        let sub = parent.expand();
        self.comms
            .iter()
            .map(|members| {
                members
                    .iter()
                    .flat_map(|&m| sub[m].iter().copied())
                    .collect()
            })
            .collect()
    }

    /// Go `(*level).localMove` (louvain.go:71): moves nodes between communities until no move
    /// improves modularity, and reports whether any node moved.
    fn local_move(&mut self, rnd: &mut dyn FnMut(usize) -> usize, look_at_all: bool) -> bool {
        let n = self.self_w.len();
        let mut degree = vec![0.0f64; n]; // weighted, internal weight included
        let mut m2 = 0.0f64;
        for (u, du) in degree.iter_mut().enumerate() {
            let mut d = self.self_w[u];
            if self.w.is_none() {
                d += (self.off[u + 1] - self.off[u]) as f64;
            }
            for &w in weights(&self.w, &self.off, u).unwrap_or_default() {
                d += w;
            }
            *du = d;
            m2 += d;
        }
        if m2 == 0.0 {
            return false;
        }

        let mut member = vec![0i32; n]; // community of each node
        let mut pos = vec![0usize; n]; // index of each node within its community
        let mut total = vec![0.0f64; self.comms.len()];
        for (c, nodes) in self.comms.iter().enumerate() {
            for (i, &u) in nodes.iter().enumerate() {
                member[u] = c as i32;
                pos[u] = i;
                total[c] += degree[u];
            }
        }
        let mut order: Vec<usize> = (0..n).collect();
        let mut to_comm = vec![0.0f64; self.comms.len()]; // weight from the node to each community
        let mut seen = vec![false; self.comms.len()];
        let mut candidates: Vec<i32> = Vec::new();

        // A node that stayed put is skipped until its verdict could differ (the long argument is
        // at louvain.go:107-117). Looking at a node more often than needed changes nothing.
        let mut dirty = vec![true; n];
        let mut own_limit = vec![0.0f64; n]; // look again when the node's own total passes this
        let mut lost_limit = vec![0.0f64; n]; // or when lost does
        let mut lost = 0.0f64; // weight that has left any community so far
        let mut cand_k: Vec<f64> = Vec::new(); // to_comm of each candidate

        let mut changed = false;
        loop {
            let mut moved = false;
            shuffle(&mut order, rnd);
            for &u in &order {
                let own = member[u];
                let own_u = own as usize;
                if !look_at_all
                    && !dirty[u]
                    && total[own_u] <= own_limit[u]
                    && lost <= lost_limit[u]
                {
                    continue;
                }
                let row = &self.nbr[self.off[u]..self.off[u + 1]];
                let w = weights(&self.w, &self.off, u);
                dirty[u] = false;
                candidates.clear();
                candidates.push(own);
                seen[own_u] = true;
                for (j, &v) in row.iter().enumerate() {
                    let c = member[v as usize];
                    let c_u = c as usize;
                    if !seen[c_u] {
                        seen[c_u] = true;
                        candidates.push(c);
                    }
                    match w {
                        None => to_comm[c_u] += 1.0,
                        Some(w) => to_comm[c_u] += w[j],
                    }
                }
                let k = degree[u];
                let mut remove = 0.0f64;
                let mut k_own = 0.0f64;
                let mut add = f64::NEG_INFINITY;
                let mut dst = -1i32;
                cand_k.clear();
                for &c in &candidates {
                    let c_u = c as usize;
                    let (kc, tot) = (to_comm[c_u], total[c_u]);
                    to_comm[c_u] = 0.0;
                    seen[c_u] = false;
                    cand_k.push(kc);
                    if c == own {
                        k_own = kc;
                        remove = kc - k * (tot - k) / m2;
                    } else {
                        let dq = kc - k * tot / m2;
                        // The lowest index wins a tie, as when gonum scans the candidates in
                        // ascending order.
                        if dq > add || (dq == add && c < dst) {
                            add = dq;
                            dst = c;
                        }
                    }
                }
                if 2.0 * (add - remove) / m2 <= DELTA_Q_TOL {
                    own_limit[u] = total[own_u];
                    lost_limit[u] = lost;
                    if dst < 0 {
                        // Nowhere to go, whatever the totals.
                        own_limit[u] = f64::INFINITY;
                        lost_limit[u] = f64::INFINITY;
                    } else {
                        let slack = ((remove - add) * m2 / k / 2.0).floor();
                        if slack >= 1.0 {
                            // Half the room real arithmetic leaves to each side, whole so the
                            // sums stay exact; kept only if the test agrees.
                            let mut worst = f64::NEG_INFINITY;
                            for (i, &c) in candidates.iter().enumerate() {
                                if c != own {
                                    worst =
                                        worst.max(cand_k[i] - k * (total[c as usize] - slack) / m2);
                                }
                            }
                            if 2.0 * (worst - (k_own - k * (total[own_u] + slack - k) / m2)) / m2
                                <= DELTA_Q_TOL
                            {
                                own_limit[u] = total[own_u] + slack;
                                lost_limit[u] = lost + slack;
                            }
                        }
                    }
                    continue;
                }
                moved = true;
                changed = true;

                // Swap-remove, as gonum does: member order decides later numbering.
                let src = &mut self.comms[own_u];
                let last = src[src.len() - 1];
                let pos_u = pos[u];
                src[pos_u] = last;
                pos[last] = pos_u;
                src.pop();
                let dst_u = dst as usize;
                member[u] = dst;
                pos[u] = self.comms[dst_u].len();
                self.comms[dst_u].push(u);
                total[own_u] -= k;
                total[dst_u] += k;
                lost += k;
                dirty[u] = true;
                for &v in row {
                    dirty[v as usize] = true;
                }
            }
            if !moved {
                return changed;
            }
        }
    }

    /// Go `(*level).reduce` (louvain.go:224): builds the next level, one node per non-empty
    /// community. Takes the level by value because the new level owns it as its parent.
    fn reduce(mut self) -> Level {
        // Fill each empty slot from the end, as gonum does: it fixes the numbering.
        let mut i = 0;
        while i < self.comms.len() {
            if self.comms[i].is_empty() {
                self.comms.swap_remove(i);
            } else {
                i += 1;
            }
        }

        let n = self.comms.len();
        let mut r = Level {
            off: vec![0; n + 1],
            nbr: Vec::new(),
            w: None,
            self_w: vec![0.0; n],
            comms: vec![Vec::new(); n],
            parent: None,
        };
        let mut rw: Vec<f64> = Vec::new();
        let mut of = vec![0i32; self.self_w.len()];
        for (c, nodes) in self.comms.iter().enumerate() {
            r.comms[c] = vec![c];
            for &u in nodes {
                of[u] = c as i32;
            }
        }
        let mut slot = vec![0i32; n]; // 1-based index into r.nbr, set only for the row being built
        for (c, nodes) in self.comms.iter().enumerate() {
            let start = r.nbr.len();
            for &u in nodes {
                r.self_w[c] += self.self_w[u];
                let w = weights(&self.w, &self.off, u);
                for (j, &v) in self.nbr[self.off[u]..self.off[u + 1]].iter().enumerate() {
                    let x = w.map_or(1.0, |w| w[j]);
                    let vc = of[v as usize];
                    let vc_u = vc as usize;
                    if vc_u == c {
                        r.self_w[c] += x; // seen from both ends, so 2w per pair
                        continue;
                    }
                    if slot[vc_u] == 0 {
                        r.nbr.push(vc);
                        rw.push(0.0);
                        slot[vc_u] = r.nbr.len() as i32;
                    }
                    rw[slot[vc_u] as usize - 1] += x;
                }
            }
            for &vc in &r.nbr[start..] {
                slot[vc as usize] = 0;
            }
            r.off[c + 1] = r.nbr.len();
        }
        r.w = Some(rw);
        r.parent = Some(Box::new(self));
        r
    }
}
