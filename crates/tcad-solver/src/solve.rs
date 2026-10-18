//! Solving: independent clusters, Newton steps with Levenberg-Marquardt damping as the fallback,
//! conflict isolation and degree-of-freedom analysis.

use crate::dual::Dual;
use crate::equation::residual;
use crate::linalg::{Sparse, lm_step, row_rank};
use crate::problem::{Equation, Problem, Var};
use std::collections::BTreeSet;

/// Residual below which iteration stops.
const STOP_TOL: f64 = 1e-11;
/// Residual below which a cluster counts as solved (millimetres or radians).
pub const SOLVED_TOL: f64 = 1e-8;
const MAX_ITERS: usize = 100;
const MAX_DAMPING_TRIES: usize = 30;

/// A set of equations that share variables, solved independently of all others.
#[derive(Clone, Debug)]
pub struct Cluster {
    /// Indices into [`Problem::equations`].
    pub equations: Vec<usize>,
    /// Variables of the cluster, ascending.
    pub vars: Vec<Var>,
}

/// How solving one cluster went.
#[derive(Clone, Debug)]
pub struct ClusterResult {
    /// Variables of the cluster.
    pub vars: Vec<Var>,
    /// Number of equations.
    pub equations: usize,
    /// True if every residual is within [`SOLVED_TOL`].
    pub converged: bool,
    /// Largest remaining residual.
    pub residual: f64,
    /// Iterations used.
    pub iterations: usize,
}

/// A minimal set of constraints that cannot all be satisfied (near the starting position).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    /// Constraint tags, ascending. Empty if the geometry itself is infeasible.
    pub tags: Vec<usize>,
}

/// The result of [`Problem::solve`].
#[derive(Clone, Debug)]
pub struct Solution {
    /// Variable values (the starting values for variables no equation touches).
    pub x: Vec<f64>,
    /// One entry per cluster.
    pub clusters: Vec<ClusterResult>,
    /// Conflicts found in clusters that did not converge.
    pub conflicts: Vec<Conflict>,
}

impl Solution {
    /// True if every cluster converged.
    pub fn ok(&self) -> bool {
        self.clusters.iter().all(|c| c.converged)
    }
}

/// Degrees of freedom and redundancy.
#[derive(Clone, Debug)]
pub struct Analysis {
    /// Total degrees of freedom of the sketch (including rigid motion if nothing is fixed).
    pub dof: usize,
    /// Degrees of freedom per registered entity, in registration order.
    pub entity_dof: Vec<usize>,
    /// Tags of constraints that add nothing to the ones before them.
    pub redundant: Vec<usize>,
}

impl Analysis {
    /// True if nothing can move.
    pub fn fully_constrained(&self) -> bool {
        self.dof == 0
    }
}

fn norm_inf(v: &[f64]) -> f64 {
    v.iter().fold(0.0_f64, |a, b| a.max(b.abs()))
}

fn cost(v: &[f64]) -> f64 {
    0.5 * v.iter().map(|x| x * x).sum::<f64>()
}

impl Problem {
    /// Splits the equations into independent clusters, ordered by their smallest variable. With
    /// `respect_pins` pinned variables are constants: they neither couple equations nor appear
    /// in a cluster's variables.
    pub fn clusters(&self, respect_pins: bool) -> Vec<Cluster> {
        let n = self.x0.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(parent: &mut [usize], mut a: usize) -> usize {
            while parent[a] != a {
                parent[a] = parent[parent[a]];
                a = parent[a];
            }
            a
        }
        for eq in &self.equations {
            let free: Vec<Var> = eq
                .vars
                .iter()
                .copied()
                .filter(|&v| !(respect_pins && self.pinned[v]))
                .collect();
            if let Some(&first) = free.first() {
                for &v in &free[1..] {
                    let (a, b) = (find(&mut parent, first), find(&mut parent, v));
                    if a != b {
                        parent[a.max(b)] = a.min(b);
                    }
                }
            }
        }
        let mut by_root: std::collections::BTreeMap<usize, Cluster> = Default::default();
        for (i, eq) in self.equations.iter().enumerate() {
            let key = match eq
                .vars
                .iter()
                .copied()
                .find(|&v| !(respect_pins && self.pinned[v]))
            {
                Some(first) => find(&mut parent, first),
                // an equation of pinned variables only is its own cluster
                None => n + i,
            };
            by_root
                .entry(key)
                .or_insert_with(|| Cluster {
                    equations: Vec::new(),
                    vars: Vec::new(),
                })
                .equations
                .push(i);
        }
        let mut out: Vec<Cluster> = by_root.into_values().collect();
        for c in &mut out {
            let set: BTreeSet<Var> = c
                .equations
                .iter()
                .flat_map(|&i| self.equations[i].vars.iter().copied())
                .filter(|&v| !(respect_pins && self.pinned[v]))
                .collect();
            c.vars = set.into_iter().collect();
        }
        out.sort_by_key(|c| c.vars.first().copied().unwrap_or(usize::MAX));
        out
    }

    fn eval_residuals(&self, eqs: &[usize], x: &[f64]) -> Vec<f64> {
        eqs.iter()
            .map(|&i| {
                let e = &self.equations[i];
                let v: Vec<Dual> = e.vars.iter().map(|&k| Dual::constant(x[k])).collect();
                residual(&e.kind, &v).v
            })
            .collect()
    }

    /// Residuals and the sparse Jacobian over the given local column numbering; variables with
    /// no local column (pinned ones) are constants.
    fn linearize(
        &self,
        eqs: &[usize],
        x: &[f64],
        local: &[usize],
        ncols: usize,
    ) -> (Vec<f64>, Sparse) {
        let mut r = Vec::with_capacity(eqs.len());
        let mut rows = Vec::with_capacity(eqs.len());
        for &i in eqs {
            let e: &Equation = &self.equations[i];
            let v: Vec<Dual> = e
                .vars
                .iter()
                .enumerate()
                .map(|(k, &g)| Dual::var(x[g], k))
                .collect();
            let d = residual(&e.kind, &v);
            r.push(d.v);
            // A variable can occur several times in one equation (a shared point); merge the
            // entries so every row has at most one entry per column.
            let mut row: Vec<(usize, f64)> = e
                .vars
                .iter()
                .enumerate()
                .filter(|&(_, &g)| local[g] != usize::MAX)
                .map(|(k, &g)| (local[g], d.d[k]))
                .collect();
            row.sort_by_key(|&(c, _)| c);
            let mut merged: Vec<(usize, f64)> = Vec::with_capacity(row.len());
            for (c, v) in row {
                match merged.last_mut() {
                    Some(last) if last.0 == c => last.1 += v,
                    _ => merged.push((c, v)),
                }
            }
            rows.push(merged);
        }
        (r, Sparse { rows, ncols })
    }

    fn local_index(&self, vars: &[Var]) -> Vec<usize> {
        let mut local = vec![usize::MAX; self.x0.len()];
        for (i, &v) in vars.iter().enumerate() {
            local[v] = i;
        }
        local
    }

    /// Solves a set of equations in place over the free variables `vars`; returns
    /// `(largest residual, iterations)`.
    fn solve_equations(
        &self,
        eqs: &[usize],
        vars: &[Var],
        x: &mut [f64],
        weighted: bool,
    ) -> (f64, usize) {
        let local = self.local_index(vars);
        let n = vars.len();
        // Column scaling by 1/sqrt(weight) makes the minimum-norm step minimize weighted movement.
        let inv_sqrt_w: Vec<f64> = vars
            .iter()
            .map(|&v| {
                if weighted {
                    1.0 / self.weights[v].sqrt()
                } else {
                    1.0
                }
            })
            .collect();
        let mut r = self.eval_residuals(eqs, x);
        if n == 0 {
            return (norm_inf(&r), 0);
        }
        let mut c = cost(&r);
        let mut lambda = 0.0_f64;
        let mut iters = 0;
        while iters < MAX_ITERS && norm_inf(&r) > STOP_TOL {
            iters += 1;
            let (_, mut jac) = self.linearize(eqs, x, &local, n);
            for row in &mut jac.rows {
                for (c, v) in row.iter_mut() {
                    *v *= inv_sqrt_w[*c];
                }
            }
            let neg: Vec<f64> = r.iter().map(|v| -v).collect();
            let mut accepted = false;
            for _ in 0..MAX_DAMPING_TRIES {
                let mut dx = lm_step(&jac, &neg, lambda);
                dx.iter_mut().zip(&inv_sqrt_w).for_each(|(d, s)| *d *= s);
                // Backtrack along the step first (cheap: residuals only); damp harder only if
                // even a short step does not help.
                let mut alpha = 1.0;
                for _ in 0..6 {
                    let mut trial = x.to_vec();
                    for (i, &v) in vars.iter().enumerate() {
                        trial[v] += alpha * dx[i];
                    }
                    let r_new = self.eval_residuals(eqs, &trial);
                    let c_new = cost(&r_new);
                    if c_new.is_finite() && c_new < c {
                        let step = alpha * norm_inf(&dx);
                        x.copy_from_slice(&trial);
                        r = r_new;
                        let tiny = step < 1e-15 || (c - c_new) < 1e-32;
                        c = c_new;
                        if alpha == 1.0 {
                            lambda = if lambda <= 1e-8 { 0.0 } else { lambda * 0.1 };
                        }
                        accepted = true;
                        if tiny {
                            return (norm_inf(&r), iters);
                        }
                        break;
                    }
                    alpha *= 0.5;
                }
                if accepted {
                    break;
                }
                lambda = if lambda == 0.0 { 1e-3 } else { lambda * 10.0 };
            }
            if !accepted {
                break;
            }
        }
        (norm_inf(&r), iters)
    }

    fn solve_clusters(
        &self,
        respect_pins: bool,
        weighted: bool,
    ) -> (Vec<f64>, Vec<ClusterResult>, Vec<Cluster>) {
        let mut x = self.x0.clone();
        let clusters = self.clusters(respect_pins);
        let mut results = Vec::with_capacity(clusters.len());
        for cl in &clusters {
            let (res, iters) = self.solve_equations(&cl.equations, &cl.vars, &mut x, weighted);
            results.push(ClusterResult {
                vars: cl.vars.clone(),
                equations: cl.equations.len(),
                converged: res <= SOLVED_TOL,
                residual: res,
                iterations: iters,
            });
        }
        (x, results, clusters)
    }

    /// Solves the problem starting from the initial values. Clusters are independent, so a
    /// failing cluster does not disturb the others.
    ///
    /// Pinned variables are first held exactly; only if that makes the problem unsolvable are they
    /// released (with their soft weights) and the problem solved again.
    pub fn solve(&self) -> Solution {
        if self.pinned.iter().any(|&p| p) {
            let (x, clusters, _) = self.solve_clusters(true, true);
            if clusters.iter().all(|c| c.converged) {
                return Solution {
                    x,
                    clusters,
                    conflicts: Vec::new(),
                };
            }
        }
        // Weights only choose between solutions; if the weighted problem is badly scaled and does
        // not converge, the unweighted one must give the same verdict.
        let mut attempt = self.solve_clusters(false, true);
        if !attempt.1.iter().all(|c| c.converged) {
            let plain = self.solve_clusters(false, false);
            let better = plain.1.iter().filter(|c| c.converged).count()
                > attempt.1.iter().filter(|c| c.converged).count();
            if better {
                attempt = plain;
            }
        }
        let (x, clusters, cls) = attempt;
        let conflicts = clusters
            .iter()
            .zip(&cls)
            .filter(|(r, _)| !r.converged)
            .map(|(_, cl)| self.isolate_conflict(cl))
            .collect();
        Solution {
            x,
            clusters,
            conflicts,
        }
    }

    fn feasible(&self, eqs: &[usize], vars: &[Var]) -> bool {
        let mut x = self.x0.clone();
        self.solve_equations(eqs, vars, &mut x, false).0 <= SOLVED_TOL
    }

    /// Finds a minimal set of constraints of a failing cluster that cannot be satisfied together
    /// (deletion filter: drop each constraint in turn and keep it dropped if the rest is still
    /// infeasible). Pins play no part: feasibility does not depend on where things start.
    fn isolate_conflict(&self, cl: &Cluster) -> Conflict {
        let tags: BTreeSet<usize> = cl
            .equations
            .iter()
            .filter_map(|&i| self.equations[i].tag)
            .collect();
        let mut keep: Vec<usize> = tags.iter().copied().collect();
        for t in tags {
            let trial: Vec<usize> = keep.iter().copied().filter(|&k| k != t).collect();
            let eqs: Vec<usize> = cl
                .equations
                .iter()
                .copied()
                .filter(|&i| self.equations[i].tag.is_none_or(|g| trial.contains(&g)))
                .collect();
            if !self.feasible(&eqs, &cl.vars) {
                keep = trial;
            }
        }
        Conflict { tags: keep }
    }

    /// Degrees of freedom per entity and redundant constraints, evaluated at a solution. Pins are
    /// ignored: a pinned coordinate is still free until a constraint holds it.
    ///
    /// With `J` the Jacobian of a cluster, the cluster has `n - rank(J)` degrees of freedom, and an
    /// entity with variables `V` has `|V| - rank(J) + rank(J without the columns of V)` of them
    /// (the dimension of the null space of `J` projected onto `V`). Ranks come from [`row_rank`].
    pub fn analyze(&self, sol: &Solution) -> Analysis {
        let mut dof = 0;
        let mut entity_dof = vec![0; self.entities.len()];
        let mut redundant: BTreeSet<usize> = BTreeSet::new();
        let mut in_cluster = vec![false; self.x0.len()];
        for cl in self.clusters(false) {
            let local = self.local_index(&cl.vars);
            let n = cl.vars.len();
            let (_, jac) = self.linearize(&cl.equations, &sol.x, &local, n);
            for &v in &cl.vars {
                in_cluster[v] = true;
            }
            let (rank, dependent) = row_rank(&jac, &|_| true);
            // A constraint is redundant if every one of its equations depends on earlier ones.
            let mut independent_by_tag: std::collections::BTreeMap<usize, bool> =
                Default::default();
            for (k, &ei) in cl.equations.iter().enumerate() {
                if let Some(tag) = self.equations[ei].tag {
                    *independent_by_tag.entry(tag).or_insert(false) |= !dependent[k];
                }
            }
            redundant.extend(
                independent_by_tag
                    .into_iter()
                    .filter(|(_, ind)| !ind)
                    .map(|(t, _)| t),
            );
            let nullity = n - rank;
            dof += nullity;
            if nullity == 0 {
                continue;
            }
            for (ei, ent) in self.entities.iter().enumerate() {
                let cols: Vec<usize> = ent
                    .vars
                    .iter()
                    .filter(|&&v| local[v] != usize::MAX)
                    .map(|&v| local[v])
                    .collect();
                if cols.is_empty() {
                    continue;
                }
                let mut seen = cols.clone();
                seen.sort_unstable();
                seen.dedup();
                let (rank_without, _) = row_rank(&jac, &|c| seen.binary_search(&c).is_err());
                entity_dof[ei] += seen.len() + rank_without - rank;
            }
        }
        // Variables no equation touches are free.
        for (ei, ent) in self.entities.iter().enumerate() {
            let free: BTreeSet<Var> = ent
                .vars
                .iter()
                .copied()
                .filter(|&v| !in_cluster[v])
                .collect();
            entity_dof[ei] += free.len();
        }
        dof += in_cluster.iter().filter(|c| !**c).count();
        Analysis {
            dof,
            entity_dof,
            redundant: redundant.into_iter().collect(),
        }
    }
}
