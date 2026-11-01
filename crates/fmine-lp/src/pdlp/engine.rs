//! Scaling and the restarted PDHG loop behind `pdlp::solve`.
//! Determinism: DeterministicPerIsa (fixed-order sums at any thread count; `sqrt` and `powf`).

use std::ops::Range;

use fmine_evidence::Refusal;

use super::mat::{Mat, chunks, even_pieces, fold, nnz_pieces, run, spmv, workers};
use super::{LpProblem, PdlpOptions, PdlpStatus, Progress, refuse};

const RUIZ_ROUNDS: usize = 10;
const EVAL_EVERY: u64 = 64;
const RESTART_SUFFICIENT: f64 = 0.2;
const RESTART_NECESSARY: f64 = 0.8;
const RESTART_ARTIFICIAL: f64 = 0.36;
/// Restart displacements below this leave the primal weight alone.
const WEIGHT_MIN_MOVE: f64 = 1e-10;

/// The problem after scaling: `K = D_r A D_c`, `x = D_c x̃`, `y = D_r ỹ`.
struct Scaled {
    k: Mat,
    kt: Mat,
    c: Vec<f64>,
    lo: Vec<f64>,
    hi: Vec<f64>,
    l: Vec<f64>,
    u: Vec<f64>,
    dr: Vec<f64>,
    dc: Vec<f64>,
}

fn inverse_root(v: f64) -> f64 {
    if v > 0.0 { 1.0 / v.sqrt() } else { 1.0 }
}

/// Ruiz equilibration (`RUIZ_ROUNDS` rounds on the max norm), then one Pock–Chambolle round (α = 1).
fn scale(p: &LpProblem) -> Scaled {
    let mut k = p.a.clone();
    let (m, n) = (k.nrows(), k.ncols);
    let mut dr = vec![1.0f64; m];
    let mut dc = vec![1.0f64; n];
    for round in 0..=RUIZ_ROUNDS {
        let pock = round == RUIZ_ROUNDS;
        let mut rn = vec![0.0f64; m];
        let mut cn = vec![0.0f64; n];
        for (r, rv) in rn.iter_mut().enumerate() {
            let (cols, vals) = k.row(r);
            for (&j, &v) in cols.iter().zip(vals) {
                let a = v.abs();
                let cj = &mut cn[j as usize];
                if pock {
                    *rv += a;
                    *cj += a;
                } else {
                    *rv = rv.max(a);
                    *cj = cj.max(a);
                }
            }
        }
        let rs: Vec<f64> = rn.iter().map(|&v| inverse_root(v)).collect();
        let cs: Vec<f64> = cn.iter().map(|&v| inverse_root(v)).collect();
        for (r, &s) in rs.iter().enumerate() {
            for e in k.ptr[r]..k.ptr[r + 1] {
                let j = k.idx[e] as usize;
                k.val[e] *= s * cs[j];
            }
        }
        for (d, s) in dr.iter_mut().zip(&rs) {
            *d *= s;
        }
        for (d, s) in dc.iter_mut().zip(&cs) {
            *d *= s;
        }
    }
    let kt = k.transpose();
    let c = p.c.iter().zip(&dc).map(|(c, d)| c * d).collect();
    let lo = p.row_lo.iter().zip(&dr).map(|(b, d)| b * d).collect();
    let hi = p.row_hi.iter().zip(&dr).map(|(b, d)| b * d).collect();
    let l = p.col_lo.iter().zip(&dc).map(|(b, d)| b / d).collect();
    let u = p.col_hi.iter().zip(&dc).map(|(b, d)| b / d).collect();
    Scaled {
        k,
        kt,
        c,
        lo,
        hi,
        l,
        u,
        dr,
        dc,
    }
}

/// Each row's largest finite bound magnitude, as a 2-norm over rows.
fn bound_norm(lo: &[f64], hi: &[f64]) -> f64 {
    let mut s = 0.0f64;
    for (&l, &h) in lo.iter().zip(hi) {
        let mut b = 0.0f64;
        for v in [l, h] {
            if v.is_finite() {
                b = b.max(v.abs());
            }
        }
        s += b * b;
    }
    s.sqrt()
}

fn norm(v: &[f64]) -> f64 {
    v.iter().map(|a| a * a).sum::<f64>().sqrt()
}

/// One point's optimality measures: scaled residuals for the restart rule, unscaled relative ones for stopping.
#[derive(Debug, Clone, Copy)]
struct Measures {
    p2s: f64,
    d2s: f64,
    pobj: f64,
    dobj: f64,
    rel_p: f64,
    rel_d: f64,
    rel_gap: f64,
}

impl Measures {
    fn kkt(&self, omega: f64) -> f64 {
        let gap = self.pobj - self.dobj;
        (omega * self.p2s + self.d2s / omega + gap * gap).sqrt()
    }

    fn converged(&self, eps: f64) -> bool {
        self.rel_p <= eps && self.rel_d <= eps && self.rel_gap <= eps
    }

    fn finite(&self) -> bool {
        [self.p2s, self.d2s, self.pobj, self.dobj]
            .iter()
            .all(|v| v.is_finite())
    }
}

/// A point `(x, y)` with its products `Kx` and `Kᵀy`, each entry multiplied by `mult` as it is read (the
/// average is stored as weighted sums).
struct Point<'a> {
    x: &'a [f64],
    y: &'a [f64],
    kx: &'a [f64],
    kty: &'a [f64],
    mult: f64,
}

struct Solver<'a> {
    s: Scaled,
    rows: Vec<Range<usize>>,
    cols: Vec<Range<usize>>,
    kp: Vec<Range<usize>>,
    ktp: Vec<Range<usize>>,
    bnorm: f64,
    cnorm: f64,
    p: &'a LpProblem,
}

impl Solver<'_> {
    fn measure(&self, pt: &Point<'_>) -> Result<Measures, Refusal> {
        let s = &self.s;
        let m = pt.mult;
        let row_parts = run(&self.rows, (), |range, ()| {
            chunks(range)
                .map(|ch| {
                    let (mut p2s, mut p2u, mut g) = (0.0f64, 0.0f64, 0.0f64);
                    for i in ch {
                        let (kxi, yi) = (pt.kx[i] * m, pt.y[i] * m);
                        let viol = (s.lo[i] - kxi).max(kxi - s.hi[i]).max(0.0);
                        p2s += viol * viol;
                        let vu = viol / s.dr[i];
                        p2u += vu * vu;
                        if yi > 0.0 && s.lo[i].is_finite() {
                            g += yi * s.lo[i];
                        } else if yi < 0.0 && s.hi[i].is_finite() {
                            g += yi * s.hi[i];
                        }
                    }
                    [p2s, p2u, g]
                })
                .collect::<Vec<_>>()
        })?;
        let col_parts = run(&self.cols, (), |range, ()| {
            chunks(range)
                .map(|ch| {
                    let (mut d2s, mut d2u, mut dcol, mut pobj) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
                    for j in ch {
                        let xj = pt.x[j] * m;
                        let r = s.c[j] - pt.kty[j] * m;
                        pobj += s.c[j] * xj;
                        let end = if r > 0.0 {
                            s.l[j]
                        } else if r < 0.0 {
                            s.u[j]
                        } else {
                            0.0
                        };
                        if end.is_finite() {
                            dcol += r * end;
                        } else {
                            d2s += r * r;
                            let ru = r / s.dc[j];
                            d2u += ru * ru;
                        }
                    }
                    [d2s, d2u, dcol, pobj]
                })
                .collect::<Vec<_>>()
        })?;
        let [p2s, p2u, g] = fold(row_parts);
        let [d2s, d2u, dcol, pobj] = fold(col_parts);
        let dobj = g + dcol;
        Ok(Measures {
            p2s,
            d2s,
            pobj,
            dobj,
            rel_p: p2u.sqrt() / (1.0 + self.bnorm),
            rel_d: d2u.sqrt() / (1.0 + self.cnorm),
            rel_gap: (pobj - dobj).abs() / (1.0 + pobj.abs() + dobj.abs()),
        })
    }

    /// The unscaled point, projected onto the column bounds against rounding in the unscaling.
    fn unscale(&self, pt: &Point<'_>) -> (Vec<f64>, Vec<f64>) {
        let s = &self.s;
        let x =
            pt.x.iter()
                .zip(&s.dc)
                .zip(self.p.col_lo.iter().zip(&self.p.col_hi))
                .map(|((v, d), (l, u))| (v * pt.mult * d).max(*l).min(*u))
                .collect();
        let y =
            pt.y.iter()
                .zip(&s.dr)
                .map(|(v, d)| v * pt.mult * d)
                .collect();
        (x, y)
    }
}

type Outcome = (PdlpStatus, Vec<f64>, Vec<f64>, Progress);

/// Runs PDLP on `p`.
pub(super) fn run_pdlp(
    p: &LpProblem,
    o: &PdlpOptions,
    observer: &mut dyn FnMut(&Progress) -> bool,
) -> Result<Outcome, Refusal> {
    let s = scale(p);
    let (m, n) = (p.rows(), p.cols());
    let w = workers(o.threads, p.nnz());
    let solver = Solver {
        rows: even_pieces(m, w),
        cols: even_pieces(n, w),
        kp: nnz_pieces(&s.k, w),
        ktp: nnz_pieces(&s.kt, w),
        bnorm: bound_norm(&p.row_lo, &p.row_hi),
        cnorm: norm(&p.c),
        s,
        p,
    };
    let s = &solver.s;

    let mut x: Vec<f64> =
        s.l.iter()
            .zip(&s.u)
            .map(|(l, u)| 0.0f64.max(*l).min(*u))
            .collect();
    let mut y = vec![0.0f64; m];
    let mut kx = vec![0.0f64; m];
    spmv(&s.k, &solver.kp, &x, &mut kx)?;
    let mut kty = vec![0.0f64; n];
    let (mut xn, mut yn, mut kxn, mut ktyn) = (
        vec![0.0f64; n],
        vec![0.0f64; m],
        vec![0.0f64; m],
        vec![0.0f64; n],
    );
    let (mut sx, mut sy, mut skx, mut skty) = (
        vec![0.0f64; n],
        vec![0.0f64; m],
        vec![0.0f64; m],
        vec![0.0f64; n],
    );
    let mut wsum = 0.0f64;

    let cn = norm(&s.c);
    let bn = bound_norm(&s.lo, &s.hi);
    let mut omega = if cn > WEIGHT_MIN_MOVE && bn > WEIGHT_MIN_MOVE {
        cn / bn
    } else {
        1.0
    };
    let kmax = s.k.val.iter().fold(0.0f64, |a, v| a.max(v.abs()));
    let mut eta = if kmax > 0.0 { 1.0 / kmax } else { 1.0 };

    let (mut iters, mut restarts, mut since_restart) = (0u64, 0u64, 0u64);
    // Step attempts, rejected ones included, counted from 1: PDLP's `k` in its step-size rule.
    let mut attempts = 0u64;
    let start = solver.measure(&Point {
        x: &x,
        y: &y,
        kx: &kx,
        kty: &kty,
        mult: 1.0,
    })?;
    let mut kkt_restart = start.kkt(omega);
    let mut prev_candidate = f64::INFINITY;
    let (mut x0, mut y0) = (x.clone(), y.clone());
    let eps = o.tolerance;

    let progress = |me: &Measures, iters: u64, restarts: u64, eta: f64, omega: f64| Progress {
        iterations: iters,
        restarts,
        primal_objective: me.pobj,
        dual_objective: me.dobj,
        relative_primal_residual: me.rel_p,
        relative_dual_residual: me.rel_d,
        relative_gap: me.rel_gap,
        step_size: eta,
        primal_weight: omega,
    };

    let status = loop {
        if iters >= o.max_iterations {
            break PdlpStatus::IterationLimit;
        }
        // One accepted PDHG step, shrinking eta until the adaptive rule accepts it.
        let used = loop {
            let (tau, sigma) = (eta / omega, eta * omega);
            let dx2 = fold(run(&solver.cols, &mut xn[..], |range, out: &mut [f64]| {
                let base = range.start;
                chunks(range)
                    .map(|ch| {
                        let mut acc = 0.0f64;
                        for j in ch {
                            let v = (x[j] - tau * (s.c[j] - kty[j])).max(s.l[j]).min(s.u[j]);
                            out[j - base] = v;
                            let d = v - x[j];
                            acc += d * d;
                        }
                        [acc]
                    })
                    .collect::<Vec<_>>()
            })?)[0];
            spmv(&s.k, &solver.kp, &xn, &mut kxn)?;
            let dy2 = fold(run(&solver.rows, &mut yn[..], |range, out: &mut [f64]| {
                let base = range.start;
                chunks(range)
                    .map(|ch| {
                        let mut acc = 0.0f64;
                        for i in ch {
                            let wv = y[i] - sigma * (2.0 * kxn[i] - kx[i]);
                            let at_lo = wv + sigma * s.lo[i];
                            let at_hi = wv + sigma * s.hi[i];
                            let v = if at_lo > 0.0 {
                                at_lo
                            } else if at_hi < 0.0 {
                                at_hi
                            } else {
                                0.0
                            };
                            out[i - base] = v;
                            let d = v - y[i];
                            acc += d * d;
                        }
                        [acc]
                    })
                    .collect::<Vec<_>>()
            })?)[0];
            spmv(&s.kt, &solver.ktp, &yn, &mut ktyn)?;
            let inter = fold(run(&solver.cols, (), |range, ()| {
                chunks(range)
                    .map(|ch| {
                        let mut acc = 0.0f64;
                        for j in ch {
                            acc += (xn[j] - x[j]) * (ktyn[j] - kty[j]);
                        }
                        [acc]
                    })
                    .collect::<Vec<_>>()
            })?)[0]
                .abs();
            let movement = 0.5 * (omega * dx2 + dy2 / omega);
            let limit = if inter > 0.0 {
                movement / inter
            } else {
                f64::INFINITY
            };
            attempts += 1;
            let k1 = (attempts + 1) as f64;
            let next = ((1.0 - k1.powf(-0.3)) * limit).min((1.0 + k1.powf(-0.6)) * eta);
            if eta <= limit {
                let used = eta;
                eta = next;
                break used;
            }
            eta = next;
            if !(eta.is_finite() && eta > 0.0) {
                return Err(refuse(format!(
                    "the PDHG step size became {eta} at step {iters}"
                )));
            }
        };

        run(
            &solver.cols,
            (&mut sx[..], &mut skty[..]),
            |range, (ax, akty): (&mut [f64], &mut [f64])| {
                let base = range.start;
                for j in range {
                    ax[j - base] += used * xn[j];
                    akty[j - base] += used * ktyn[j];
                }
            },
        )?;
        run(
            &solver.rows,
            (&mut sy[..], &mut skx[..]),
            |range, (ay, akx): (&mut [f64], &mut [f64])| {
                let base = range.start;
                for i in range {
                    ay[i - base] += used * yn[i];
                    akx[i - base] += used * kxn[i];
                }
            },
        )?;
        wsum += used;
        std::mem::swap(&mut x, &mut xn);
        std::mem::swap(&mut y, &mut yn);
        std::mem::swap(&mut kx, &mut kxn);
        std::mem::swap(&mut kty, &mut ktyn);
        iters += 1;
        since_restart += 1;

        if iters % EVAL_EVERY != 0 {
            continue;
        }
        let mc = solver.measure(&Point {
            x: &x,
            y: &y,
            kx: &kx,
            kty: &kty,
            mult: 1.0,
        })?;
        let avg = Point {
            x: &sx,
            y: &sy,
            kx: &skx,
            kty: &skty,
            mult: 1.0 / wsum,
        };
        let ma = solver.measure(&avg)?;
        if !(mc.finite() && ma.finite()) {
            return Err(refuse(format!(
                "the PDHG iterate became non-finite by step {iters}"
            )));
        }
        if mc.converged(eps) {
            let (xo, yo) = solver.unscale(&Point {
                x: &x,
                y: &y,
                kx: &kx,
                kty: &kty,
                mult: 1.0,
            });
            return Ok((
                PdlpStatus::Optimal,
                xo,
                yo,
                progress(&mc, iters, restarts, eta, omega),
            ));
        }
        if ma.converged(eps) {
            let (xo, yo) = solver.unscale(&avg);
            return Ok((
                PdlpStatus::Optimal,
                xo,
                yo,
                progress(&ma, iters, restarts, eta, omega),
            ));
        }
        let (kc, ka) = (mc.kkt(omega), ma.kkt(omega));
        let use_avg = ka < kc;
        let (cand, cand_kkt) = if use_avg { (ma, ka) } else { (mc, kc) };
        if !observer(&progress(&cand, iters, restarts, eta, omega)) {
            let (xo, yo) = if use_avg {
                solver.unscale(&avg)
            } else {
                solver.unscale(&Point {
                    x: &x,
                    y: &y,
                    kx: &kx,
                    kty: &kty,
                    mult: 1.0,
                })
            };
            return Ok((
                PdlpStatus::Stopped,
                xo,
                yo,
                progress(&cand, iters, restarts, eta, omega),
            ));
        }
        let restart = cand_kkt <= RESTART_SUFFICIENT * kkt_restart
            || (cand_kkt <= RESTART_NECESSARY * kkt_restart && cand_kkt > prev_candidate)
            || since_restart as f64 >= RESTART_ARTIFICIAL * iters as f64;
        if !restart {
            prev_candidate = cand_kkt;
            continue;
        }
        if use_avg {
            let inv = 1.0 / wsum;
            for (a, b) in x.iter_mut().zip(&sx) {
                *a = b * inv;
            }
            for (a, b) in y.iter_mut().zip(&sy) {
                *a = b * inv;
            }
            for (a, b) in kx.iter_mut().zip(&skx) {
                *a = b * inv;
            }
            for (a, b) in kty.iter_mut().zip(&skty) {
                *a = b * inv;
            }
        }
        let dxr = x
            .iter()
            .zip(&x0)
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f64>()
            .sqrt();
        let dyr = y
            .iter()
            .zip(&y0)
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f64>()
            .sqrt();
        if dxr > WEIGHT_MIN_MOVE && dyr > WEIGHT_MIN_MOVE {
            omega = (omega * dyr / dxr).sqrt();
        }
        x0.copy_from_slice(&x);
        y0.copy_from_slice(&y);
        for v in [&mut sx, &mut sy, &mut skx, &mut skty] {
            v.iter_mut().for_each(|e| *e = 0.0);
        }
        wsum = 0.0;
        kkt_restart = cand.kkt(omega);
        prev_candidate = f64::INFINITY;
        restarts += 1;
        since_restart = 0;
    };

    let cur = Point {
        x: &x,
        y: &y,
        kx: &kx,
        kty: &kty,
        mult: 1.0,
    };
    let mc = solver.measure(&cur)?;
    if wsum > 0.0 {
        let avg = Point {
            x: &sx,
            y: &sy,
            kx: &skx,
            kty: &skty,
            mult: 1.0 / wsum,
        };
        let ma = solver.measure(&avg)?;
        if ma.kkt(omega) < mc.kkt(omega) {
            let (xo, yo) = solver.unscale(&avg);
            return Ok((status, xo, yo, progress(&ma, iters, restarts, eta, omega)));
        }
    }
    let (xo, yo) = solver.unscale(&cur);
    Ok((status, xo, yo, progress(&mc, iters, restarts, eta, omega)))
}
