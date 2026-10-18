//! Linear algebra: a sparse row matrix, damped CGLS for least-squares steps, and dense
//! elimination for rank and null spaces. All loops run in a fixed order, so results are
//! reproducible.

/// A sparse matrix stored by rows of `(column, value)`.
#[derive(Clone, Debug, Default)]
pub struct Sparse {
    /// Rows.
    pub rows: Vec<Vec<(usize, f64)>>,
    /// Number of columns.
    pub ncols: usize,
}

impl Sparse {
    /// `out = A x`.
    pub fn mul(&self, x: &[f64], out: &mut [f64]) {
        for (o, row) in out.iter_mut().zip(&self.rows) {
            *o = row.iter().map(|&(c, v)| v * x[c]).sum();
        }
    }

    /// `out = A^T y`.
    pub fn mul_t(&self, y: &[f64], out: &mut [f64]) {
        out.iter_mut().for_each(|o| *o = 0.0);
        for (yi, row) in y.iter().zip(&self.rows) {
            for &(c, v) in row {
                out[c] += v * yi;
            }
        }
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Minimizes `|A x - b|^2 + lambda^2 |x|^2` by conjugate gradients on the normal equations,
/// starting at zero. For `lambda = 0` and a rank-deficient `A` this converges to the minimum-norm
/// solution, which is the step to the *nearest* solution that a sketch solver wants.
pub fn cgls(a: &Sparse, b: &[f64], lambda: f64, max_iter: usize) -> Vec<f64> {
    let n = a.ncols;
    let l2 = lambda * lambda;
    let mut x = vec![0.0; n];
    let mut r = b.to_vec();
    let mut s = vec![0.0; n];
    a.mul_t(&r, &mut s);
    let mut p = s.clone();
    let mut gamma = dot(&s, &s);
    let gamma0 = gamma;
    if gamma0 == 0.0 {
        return x;
    }
    let mut q = vec![0.0; b.len()];
    for _ in 0..max_iter {
        a.mul(&p, &mut q);
        let denom = dot(&q, &q) + l2 * dot(&p, &p);
        if denom <= 0.0 {
            break;
        }
        let alpha = gamma / denom;
        for i in 0..n {
            x[i] += alpha * p[i];
        }
        for (ri, qi) in r.iter_mut().zip(&q) {
            *ri -= alpha * qi;
        }
        a.mul_t(&r, &mut s);
        for i in 0..n {
            s[i] -= l2 * x[i];
        }
        let g = dot(&s, &s);
        if g <= 1e-20 * gamma0 {
            break;
        }
        let beta = g / gamma;
        for i in 0..n {
            p[i] = s[i] + beta * p[i];
        }
        gamma = g;
    }
    x
}

/// Reverse Cuthill-McKee ordering of the graph given by adjacency lists: `order[new] = old`.
/// Deterministic: ties are broken by degree, then index.
pub fn rcm_order(adj: &[Vec<usize>]) -> Vec<usize> {
    let n = adj.len();
    let mut visited = vec![false; n];
    let mut order = Vec::with_capacity(n);
    let mut starts: Vec<usize> = (0..n).collect();
    starts.sort_by_key(|&i| (adj[i].len(), i));
    for &s in &starts {
        if visited[s] {
            continue;
        }
        visited[s] = true;
        let mut queue = std::collections::VecDeque::from([s]);
        while let Some(u) = queue.pop_front() {
            order.push(u);
            let mut next: Vec<usize> = adj[u].iter().copied().filter(|&v| !visited[v]).collect();
            next.sort_by_key(|&v| (adj[v].len(), v));
            next.dedup();
            for v in next {
                visited[v] = true;
                queue.push_back(v);
            }
        }
    }
    order.reverse();
    order
}

/// A symmetric positive definite matrix in envelope (profile) storage with its Cholesky factor.
/// After reverse Cuthill-McKee ordering the envelope of a sketch's normal equations is narrow, so
/// factoring costs about `n * bandwidth^2`.
pub struct EnvelopeCholesky {
    n: usize,
    perm: Vec<usize>,
    inv: Vec<usize>,
    first: Vec<usize>,
    start: Vec<usize>,
    data: Vec<f64>,
}

impl EnvelopeCholesky {
    /// Assembles `A^T A + mu I` for the sparse `a` and factors it. Returns `None` if the matrix
    /// is not numerically positive definite (increase `mu`).
    pub fn normal_equations(a: &Sparse, mu: f64) -> Option<Self> {
        let n = a.ncols;
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for row in &a.rows {
            for &(i, _) in row {
                for &(j, _) in row {
                    if i != j {
                        adj[i].push(j);
                    }
                }
            }
        }
        adj.iter_mut().for_each(|l| {
            l.sort_unstable();
            l.dedup();
        });
        let perm = rcm_order(&adj);
        let mut inv = vec![0; n];
        for (new, &old) in perm.iter().enumerate() {
            inv[old] = new;
        }
        let first: Vec<usize> = (0..n)
            .map(|new| {
                adj[perm[new]]
                    .iter()
                    .map(|&o| inv[o])
                    .filter(|&j| j < new)
                    .min()
                    .unwrap_or(new)
            })
            .collect();
        let mut start = Vec::with_capacity(n + 1);
        let mut total = 0;
        for i in 0..n {
            start.push(total);
            total += i - first[i] + 1;
        }
        start.push(total);
        let mut data = vec![0.0; total];
        for row in &a.rows {
            for &(ci, vi) in row {
                for &(cj, vj) in row {
                    let (i, j) = (inv[ci], inv[cj]);
                    if j <= i {
                        data[start[i] + j - first[i]] += vi * vj;
                    }
                }
            }
        }
        for i in 0..n {
            data[start[i] + i - first[i]] += mu;
        }
        let mut f = EnvelopeCholesky {
            n,
            perm,
            inv,
            first,
            start,
            data,
        };
        f.factor()?;
        Some(f)
    }

    fn at(&self, i: usize, j: usize) -> f64 {
        self.data[self.start[i] + j - self.first[i]]
    }

    fn factor(&mut self) -> Option<()> {
        for i in 0..self.n {
            for j in self.first[i]..=i {
                let lo = self.first[i].max(self.first[j]);
                let mut sum = self.at(i, j);
                for k in lo..j {
                    sum -= self.at(i, k) * self.at(j, k);
                }
                let idx = self.start[i] + j - self.first[i];
                if j < i {
                    self.data[idx] = sum / self.at(j, j);
                } else {
                    if sum <= 0.0 || !sum.is_finite() {
                        return None;
                    }
                    self.data[idx] = sum.sqrt();
                }
            }
        }
        Some(())
    }

    /// Solves `(A^T A + mu I) x = b`.
    pub fn solve(&self, b: &[f64]) -> Vec<f64> {
        let n = self.n;
        let mut y: Vec<f64> = (0..n).map(|new| b[self.perm[new]]).collect();
        for i in 0..n {
            let mut s = y[i];
            for k in self.first[i]..i {
                s -= self.at(i, k) * y[k];
            }
            y[i] = s / self.at(i, i);
        }
        for i in (0..n).rev() {
            y[i] /= self.at(i, i);
            let yi = y[i];
            for k in self.first[i]..i {
                y[k] -= self.at(i, k) * yi;
            }
        }
        (0..n).map(|old| y[self.inv[old]]).collect()
    }
}

/// The transpose of a sparse matrix with `nrows` rows.
pub fn transpose(a: &Sparse, nrows: usize) -> Sparse {
    let mut rows: Vec<Vec<(usize, f64)>> = vec![Vec::new(); a.ncols];
    for (i, row) in a.rows.iter().enumerate() {
        for &(c, v) in row {
            rows[c].push((i, v));
        }
    }
    Sparse { rows, ncols: nrows }
}

fn residual_norm(a: &Sparse, b: &[f64], x: &[f64]) -> f64 {
    let mut ax = vec![0.0; b.len()];
    a.mul(x, &mut ax);
    ax.iter()
        .zip(b)
        .map(|(p, q)| (q - p) * (q - p))
        .sum::<f64>()
        .sqrt()
}

/// The damped Gauss-Newton step `min |A x - b|^2 + lambda^2 |x|^2`.
///
/// For `m <= n` equations the step is `x = A^T (A A^T + mu I)^-1 b`, so it lies in the row space of
/// `A` *by construction*: for an under-determined sketch it is the minimum-norm step (the nearest
/// solution) with no round-off drift along directions the constraints leave free. Otherwise the
/// primal normal equations are used. Both factor an envelope-stored matrix after reverse
/// Cuthill-McKee ordering. Falls back to [`cgls`] if the factorization breaks down.
pub fn lm_step(a: &Sparse, b: &[f64], lambda: f64) -> Vec<f64> {
    // Scale every equation to unit norm: the solution of a consistent system is unchanged, but the
    // regularization below then means the same thing for every row.
    let norms: Vec<f64> = a
        .rows
        .iter()
        .map(|r| r.iter().map(|&(_, v)| v * v).sum::<f64>().sqrt())
        .collect();
    if norms.iter().any(|&n| n > 0.0 && n != 1.0) {
        let scaled = Sparse {
            rows: a
                .rows
                .iter()
                .zip(&norms)
                .map(|(r, &n)| {
                    let k = if n > 0.0 { 1.0 / n } else { 1.0 };
                    r.iter().map(|&(c, v)| (c, v * k)).collect()
                })
                .collect(),
            ncols: a.ncols,
        };
        let sb: Vec<f64> = b
            .iter()
            .zip(&norms)
            .map(|(&v, &n)| if n > 0.0 { v / n } else { v })
            .collect();
        return lm_step_scaled(&scaled, &sb, lambda);
    }
    lm_step_scaled(a, b, lambda)
}

fn lm_step_scaled(a: &Sparse, b: &[f64], lambda: f64) -> Vec<f64> {
    let (m, n) = (a.rows.len(), a.ncols);
    if b.iter().all(|v| *v == 0.0) {
        return vec![0.0; n];
    }
    let diag_max = a
        .rows
        .iter()
        .map(|r| r.iter().map(|&(_, v)| v * v).sum::<f64>())
        .fold(0.0_f64, f64::max);
    let mut mu = lambda * lambda + 1e-10 * (diag_max + 1.0);
    if m <= n {
        let at = transpose(a, m);
        let factor = (0..6).find_map(|_| {
            let f = EnvelopeCholesky::normal_equations(&at, mu);
            if f.is_none() {
                mu *= 100.0;
            }
            f
        });
        let Some(f) = factor else {
            return cgls(a, b, lambda, 4 * n + 20);
        };
        let step = |rhs: &[f64]| {
            let y = f.solve(rhs);
            let mut x = vec![0.0; n];
            a.mul_t(&y, &mut x);
            x
        };
        let mut x = step(b);
        if lambda == 0.0 {
            // Refine against the undamped residual, keeping only improvements.
            let mut best = residual_norm(a, b, &x);
            for _ in 0..4 {
                let mut ax = vec![0.0; m];
                a.mul(&x, &mut ax);
                let rr: Vec<f64> = b.iter().zip(&ax).map(|(p, q)| p - q).collect();
                let dx = step(&rr);
                let trial: Vec<f64> = x.iter().zip(&dx).map(|(p, q)| p + q).collect();
                let rn = residual_norm(a, b, &trial);
                if rn < best {
                    x = trial;
                    best = rn;
                } else {
                    break;
                }
            }
        }
        return x;
    }
    let mut g = vec![0.0; n];
    a.mul_t(b, &mut g);
    let factor = (0..6).find_map(|_| {
        let f = EnvelopeCholesky::normal_equations(a, mu);
        if f.is_none() {
            mu *= 100.0;
        }
        f
    });
    let Some(f) = factor else {
        return cgls(a, b, lambda, 4 * n + 20);
    };
    let l2 = lambda * lambda;
    let mut x = f.solve(&g);
    let gnorm = dot(&g, &g).sqrt();
    let mut ax = vec![0.0; m];
    let mut res = vec![0.0; n];
    for _ in 0..6 {
        a.mul(&x, &mut ax);
        a.mul_t(&ax, &mut res);
        for i in 0..n {
            res[i] = g[i] - res[i] - l2 * x[i];
        }
        if dot(&res, &res).sqrt() <= 1e-13 * gnorm {
            break;
        }
        let dx = f.solve(&res);
        x.iter_mut().zip(&dx).for_each(|(p, q)| *p += q);
    }
    x
}

/// Numerical rank of a sparse matrix by rows, restricted to the columns for which `include`
/// returns true, together with which rows are (numerically) combinations of the rows before them.
///
/// This is Cholesky factorization of the Gram matrix `A A^T` in envelope storage with rows in their
/// given order: the pivot of row `i` is the squared distance of that row from the span of the
/// earlier independent rows, so a tiny pivot marks a dependent row. Cost is about `m * bandwidth^2`.
/// Rows are scaled to unit length first, so the threshold is relative.
pub fn row_rank(a: &Sparse, include: &dyn Fn(usize) -> bool) -> (usize, Vec<bool>) {
    let m = a.rows.len();
    let rows: Vec<Vec<(usize, f64)>> = a
        .rows
        .iter()
        .map(|r| {
            let kept: Vec<(usize, f64)> = r.iter().copied().filter(|&(c, _)| include(c)).collect();
            let norm = kept.iter().map(|&(_, v)| v * v).sum::<f64>().sqrt();
            if norm > 0.0 {
                kept.into_iter().map(|(c, v)| (c, v / norm)).collect()
            } else {
                Vec::new()
            }
        })
        .collect();
    // rows touching each column
    let mut by_col: Vec<Vec<usize>> = vec![Vec::new(); a.ncols];
    for (i, r) in rows.iter().enumerate() {
        for &(c, _) in r {
            by_col[c].push(i);
        }
    }
    let mut first: Vec<usize> = (0..m).collect();
    for list in &by_col {
        if let Some(&lo) = list.first() {
            for &i in list {
                first[i] = first[i].min(lo);
            }
        }
    }
    let mut start = Vec::with_capacity(m + 1);
    let mut total = 0;
    for i in 0..m {
        start.push(total);
        total += i - first[i] + 1;
    }
    start.push(total);
    let mut data = vec![0.0; total];
    for list in &by_col {
        for &i in list {
            for &j in list {
                if j <= i {
                    let dot: f64 = dot_sparse(&rows[i], &rows[j]);
                    data[start[i] + j - first[i]] = dot;
                }
            }
        }
    }
    let mut dependent = vec![false; m];
    let mut rank = 0;
    for i in 0..m {
        let diag = data[start[i] + i - first[i]];
        for j in first[i]..i {
            if dependent[j] {
                data[start[i] + j - first[i]] = 0.0;
                continue;
            }
            let lo = first[i].max(first[j]);
            let mut sum = data[start[i] + j - first[i]];
            for k in lo..j {
                if !dependent[k] {
                    sum -= data[start[i] + k - first[i]] * data[start[j] + k - first[j]];
                }
            }
            data[start[i] + j - first[i]] = sum / data[start[j] + j - first[j]];
        }
        let mut pivot = diag;
        for k in first[i]..i {
            if !dependent[k] {
                let l = data[start[i] + k - first[i]];
                pivot -= l * l;
            }
        }
        if diag <= 0.0 || pivot <= 1e-12 * diag {
            dependent[i] = true;
            data[start[i] + i - first[i]] = 1.0;
        } else {
            data[start[i] + i - first[i]] = pivot.sqrt();
            rank += 1;
        }
    }
    (rank, dependent)
}

fn dot_sparse(a: &[(usize, f64)], b: &[(usize, f64)]) -> f64 {
    let mut sum = 0.0;
    for &(c, v) in a {
        if let Some(&(_, w)) = b.iter().find(|&&(d, _)| d == c) {
            sum += v * w;
        }
    }
    sum
}

/// Relative tolerance for treating an entry as zero during elimination.
const PIVOT_TOL: f64 = 1e-9;

/// Dense matrix as rows.
pub type Dense = Vec<Vec<f64>>;

/// The null space of a dense matrix (`rows x ncols`): its rank and a basis of vectors `v` with
/// `A v = 0`. Rows are scaled to unit maximum entry first.
pub fn null_space(mut rows: Dense, ncols: usize) -> (usize, Dense) {
    for row in &mut rows {
        let m = row.iter().fold(0.0_f64, |a, &b| a.max(b.abs()));
        if m > 0.0 {
            row.iter_mut().for_each(|v| *v /= m);
        }
    }
    let m = rows.len();
    let mut pivot_col_of_row: Vec<usize> = Vec::new();
    let mut r = 0;
    for c in 0..ncols {
        if r >= m {
            break;
        }
        let (best, val) = (r..m)
            .map(|i| (i, rows[i][c].abs()))
            .fold((r, 0.0), |acc, x| if x.1 > acc.1 { x } else { acc });
        if val <= PIVOT_TOL {
            continue;
        }
        rows.swap(r, best);
        let pv = rows[r][c];
        rows[r].iter_mut().for_each(|v| *v /= pv);
        let pivot_row = rows[r].clone();
        for (i, row) in rows.iter_mut().enumerate() {
            if i != r {
                let f = row[c];
                if f != 0.0 {
                    row.iter_mut()
                        .zip(&pivot_row)
                        .for_each(|(v, p)| *v -= f * p);
                }
            }
        }
        pivot_col_of_row.push(c);
        r += 1;
    }
    let rank = pivot_col_of_row.len();
    let is_pivot: Vec<bool> = {
        let mut v = vec![false; ncols];
        pivot_col_of_row.iter().for_each(|&c| v[c] = true);
        v
    };
    let basis = (0..ncols)
        .filter(|&f| !is_pivot[f])
        .map(|f| {
            let mut v = vec![0.0; ncols];
            v[f] = 1.0;
            for (i, &pc) in pivot_col_of_row.iter().enumerate() {
                v[pc] = -rows[i][f];
            }
            v
        })
        .collect();
    (rank, basis)
}

/// Rank of a dense matrix.
pub fn rank(rows: Dense, ncols: usize) -> usize {
    null_space(rows, ncols).0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sparse(rows: &[&[(usize, f64)]], ncols: usize) -> Sparse {
        Sparse {
            rows: rows.iter().map(|r| r.to_vec()).collect(),
            ncols,
        }
    }

    #[test]
    fn cgls_solves_a_square_system() {
        // [2 1; 1 3] x = [3; 5] -> x = [0.8, 1.4]
        let a = sparse(&[&[(0, 2.0), (1, 1.0)], &[(0, 1.0), (1, 3.0)]], 2);
        let x = cgls(&a, &[3.0, 5.0], 0.0, 50);
        assert!(
            (x[0] - 0.8).abs() < 1e-12 && (x[1] - 1.4).abs() < 1e-12,
            "{x:?}"
        );
    }

    #[test]
    fn cgls_gives_the_minimum_norm_solution_when_underdetermined() {
        // x + y = 2: minimum norm solution is (1, 1)
        let a = sparse(&[&[(0, 1.0), (1, 1.0)]], 2);
        let x = cgls(&a, &[2.0], 0.0, 50);
        assert!(
            (x[0] - 1.0).abs() < 1e-12 && (x[1] - 1.0).abs() < 1e-12,
            "{x:?}"
        );
    }

    #[test]
    fn lm_step_matches_cgls_including_rank_deficient_systems() {
        // a chain: x0 = 1, x_i - x_{i-1} = 1 -> x_i = i + 1
        let n = 60;
        let mut rows: Vec<Vec<(usize, f64)>> = vec![vec![(0, 1.0)]];
        for i in 1..n {
            rows.push(vec![(i, 1.0), (i - 1, -1.0)]);
        }
        let a = Sparse { rows, ncols: n };
        let mut b = vec![1.0; n];
        b[0] = 1.0;
        let x = lm_step(&a, &b, 0.0);
        for (i, v) in x.iter().enumerate() {
            assert!((v - (i as f64 + 1.0)).abs() < 1e-8, "{i}: {v}");
        }
        // under-determined: x + y = 2 -> (1, 1)
        let a = sparse(&[&[(0, 1.0), (1, 1.0)]], 2);
        let x = lm_step(&a, &[2.0], 0.0);
        assert!(
            (x[0] - 1.0).abs() < 1e-8 && (x[1] - 1.0).abs() < 1e-8,
            "{x:?}"
        );
        // damping
        let a = sparse(&[&[(0, 1.0)]], 1);
        let x = lm_step(&a, &[1.0], 1.0);
        assert!((x[0] - 0.5).abs() < 1e-8);
    }

    #[test]
    fn rcm_orders_a_path_into_a_band() {
        // a path graph numbered badly: 0-3, 3-1, 1-4, 4-2
        let adj = vec![vec![3], vec![3, 4], vec![4], vec![0, 1], vec![1, 2]];
        let order = rcm_order(&adj);
        let mut inv = [0; 5];
        order
            .iter()
            .enumerate()
            .for_each(|(new, &old)| inv[old] = new);
        for (u, ns) in adj.iter().enumerate() {
            for &v in ns {
                assert!(inv[u].abs_diff(inv[v]) == 1, "bandwidth > 1");
            }
        }
    }

    #[test]
    fn cgls_damping_shrinks_the_step() {
        let a = sparse(&[&[(0, 1.0)]], 1);
        let x = cgls(&a, &[1.0], 1.0, 50);
        assert!((x[0] - 0.5).abs() < 1e-12);
    }

    #[test]
    fn null_space_of_a_rank_deficient_matrix() {
        // rows: x - y = 0, 2x - 2y = 0 (dependent); null space is span{(1,1,0), (0,0,1)}
        let (rank, basis) = null_space(vec![vec![1.0, -1.0, 0.0], vec![2.0, -2.0, 0.0]], 3);
        assert_eq!(rank, 1);
        assert_eq!(basis.len(), 2);
        for v in &basis {
            assert!((v[0] - v[1]).abs() < 1e-12);
        }
    }

    #[test]
    fn row_rank_finds_dependent_rows_in_order() {
        // rows: x, y, x+y (dependent on the first two), x-y... (independent of nothing new in 2 columns)
        let a = sparse(
            &[&[(0, 1.0)], &[(1, 1.0)], &[(0, 1.0), (1, 1.0)], &[(0, 2.0)]],
            2,
        );
        let (rank, dep) = row_rank(&a, &|_| true);
        assert_eq!(rank, 2);
        assert_eq!(dep, [false, false, true, true]);
        // without column 1 the rows y and x+y change meaning: y vanishes
        let (rank, dep) = row_rank(&a, &|c| c == 0);
        assert_eq!(rank, 1);
        assert_eq!(dep, [false, true, true, true]);
    }

    #[test]
    fn full_rank_has_empty_null_space() {
        let (rank, basis) = null_space(vec![vec![1.0, 0.0], vec![0.0, 1.0]], 2);
        assert_eq!((rank, basis.len()), (2, 0));
    }
}
