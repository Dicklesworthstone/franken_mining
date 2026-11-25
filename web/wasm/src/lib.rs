//! The frankenmining.com figures' bridge to FrankenMining's own kernels, compiled to `wasm32-unknown-unknown`.
//!
//! Nothing here re-implements a kernel. Each export converts the page's numbers, calls the workspace crate, and
//! hands the crate's answer back:
//! - [`pit`]: `fmine-optimize` pitflow (certified ultimate pit), nested shells and TopoSort's greedy schedule;
//! - [`spatial`]: `fmine-calib`'s `SpatialFrontPitMonitor` (conditional Gaussian prediction from drillholes, and the
//!   sequential PIT e-process monitor);
//! - [`extras`]: `fmine-plant` Bond grinding, `fmine-numerics` EVPI/EVSI and CVaR.
//!
//! **Calling convention.** No wasm-bindgen and no external crates. Every export takes and returns scalars
//! (`f64`, `u32`, `i32`). Arrays travel through module-owned buffers:
//! 1. `fm_in_f64(len)` sizes the input buffer and returns its address; JS writes a `Float64Array` there.
//! 2. The compute call consumes that buffer and returns a status: 0 for success, negative for a refusal.
//! 3. Results sit in the output buffers, read through `fm_out_{f64,i32,u8}_{ptr,len}`. JS copies them out at once,
//!    since a later call may grow memory and detach every view.
//! 4. On a refusal, `fm_err_{ptr,len}` hold the UTF-8 message (the crate's `Refusal` text where there is one).
//!
//! **Unsafe.** The crate denies `unsafe_code`; every logic module forbids it. The one exception is [`exports`]: Rust
//! counts `#[unsafe(no_mangle)]`, the only way to give a wasm export a stable name, as unsafe code. That module holds
//! only one-line wrappers and no `unsafe` block.

#![deny(unsafe_code)]

pub mod extras;
pub mod pit;
pub mod spatial;

use std::sync::{Mutex, MutexGuard, PoisonError};

static IN_F64: Mutex<Vec<f64>> = Mutex::new(Vec::new());
static OUT_F64: Mutex<Vec<f64>> = Mutex::new(Vec::new());
static OUT_I32: Mutex<Vec<i32>> = Mutex::new(Vec::new());
static OUT_U8: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static ERR: Mutex<String> = Mutex::new(String::new());

/// Status for a call the bridge or the crate refused; the message is in the error buffer.
pub const REFUSED: i32 = -1;

/// The buffers are only ever touched from one thread; a poisoned lock (impossible with `panic = "abort"`) is used
/// as is rather than turned into a panic.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Takes the input buffer, leaving it empty.
pub fn take_input() -> Vec<f64> {
    std::mem::take(&mut *lock(&IN_F64))
}

/// Sizes the input buffer (zero-filled) and returns its address.
fn input_buffer(len: usize) -> usize {
    let mut buf = lock(&IN_F64);
    buf.clear();
    buf.resize(len, 0.0);
    buf.as_mut_ptr() as usize
}

/// The output buffers a call fills.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Output {
    pub f64s: Vec<f64>,
    pub i32s: Vec<i32>,
    pub u8s: Vec<u8>,
}

/// Runs one call: on success its output replaces the output buffers; on refusal the message goes to the error
/// buffer and the output buffers are emptied.
fn run(call: impl FnOnce() -> Result<Output, String>) -> i32 {
    let (status, out, err) = match call() {
        Ok(out) => (0, out, String::new()),
        Err(message) => (REFUSED, Output::default(), message),
    };
    *lock(&OUT_F64) = out.f64s;
    *lock(&OUT_I32) = out.i32s;
    *lock(&OUT_U8) = out.u8s;
    *lock(&ERR) = err;
    status
}

/// The thin, named entry points. See the crate notes for why this module allows `unsafe_code`.
#[allow(unsafe_code)]
pub mod exports {
    use super::{
        ERR, OUT_F64, OUT_I32, OUT_U8, extras, input_buffer, lock, pit, run, spatial, take_input,
    };

    // --- Buffers ---

    /// Sizes the f64 input buffer to `len` values and returns its byte address.
    #[unsafe(no_mangle)]
    pub extern "C" fn fm_in_f64(len: u32) -> u32 {
        input_buffer(len as usize) as u32
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn fm_out_f64_ptr() -> u32 {
        lock(&OUT_F64).as_ptr() as usize as u32
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn fm_out_f64_len() -> u32 {
        lock(&OUT_F64).len() as u32
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn fm_out_i32_ptr() -> u32 {
        lock(&OUT_I32).as_ptr() as usize as u32
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn fm_out_i32_len() -> u32 {
        lock(&OUT_I32).len() as u32
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn fm_out_u8_ptr() -> u32 {
        lock(&OUT_U8).as_ptr() as usize as u32
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn fm_out_u8_len() -> u32 {
        lock(&OUT_U8).len() as u32
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn fm_err_ptr() -> u32 {
        lock(&ERR).as_ptr() as usize as u32
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn fm_err_len() -> u32 {
        lock(&ERR).len() as u32
    }

    // --- fmine-optimize ---

    /// Input: `n` block values. Output f64: see [`pit::pit`].
    #[unsafe(no_mangle)]
    #[allow(clippy::too_many_arguments)]
    pub extern "C" fn fm_pit(
        nx: u32,
        ny: u32,
        nz: u32,
        dx: f64,
        dy: f64,
        dz: f64,
        rise: u32,
        run_: u32,
        levels: u32,
        scale: f64,
    ) -> i32 {
        let geo = pit::Geometry {
            counts: [nx, ny, nz],
            sizes_m: [dx, dy, dz],
            rise,
            run: run_,
            levels,
        };
        run(|| pit::pit(&geo, &take_input(), scale))
    }

    /// Input: `n` revenues, `n` costs, `factors` revenue factors. Output: see [`pit::shells`].
    #[unsafe(no_mangle)]
    #[allow(clippy::too_many_arguments)]
    pub extern "C" fn fm_shells(
        nx: u32,
        ny: u32,
        nz: u32,
        dx: f64,
        dy: f64,
        dz: f64,
        rise: u32,
        run_: u32,
        levels: u32,
        factors: u32,
        scale: f64,
    ) -> i32 {
        let geo = pit::Geometry {
            counts: [nx, ny, nz],
            sizes_m: [dx, dy, dz],
            rise,
            run: run_,
            levels,
        };
        run(|| pit::shells(&geo, &take_input(), factors as usize, scale))
    }

    /// Input: `n` values, `n` tonnages, `periods` capacities, `n` ranks (NaN: unmined; all NaN: automatic).
    /// Output: see [`pit::schedule`].
    #[unsafe(no_mangle)]
    #[allow(clippy::too_many_arguments)]
    pub extern "C" fn fm_schedule(
        nx: u32,
        ny: u32,
        nz: u32,
        dx: f64,
        dy: f64,
        dz: f64,
        rise: u32,
        run_: u32,
        levels: u32,
        periods: u32,
        scale: f64,
    ) -> i32 {
        let geo = pit::Geometry {
            counts: [nx, ny, nz],
            sizes_m: [dx, dy, dz],
            rise,
            run: run_,
            levels,
        };
        run(|| pit::schedule(&geo, &take_input(), periods as usize, scale))
    }

    // --- fmine-calib ---

    /// Input: `samples` × (x, y, z, prior, value), then `queries` × (x, y, z, prior). Output: see
    /// [`spatial::estimate`].
    #[unsafe(no_mangle)]
    pub extern "C" fn fm_estimate(
        samples: u32,
        queries: u32,
        nugget: f64,
        sill: f64,
        range_m: f64,
        neighbors: u32,
    ) -> i32 {
        let model = spatial::Model {
            nugget,
            sill,
            range_m,
            neighbors: neighbors as usize,
            alpha: 0.05,
        };
        run(|| spatial::estimate(&model, &take_input(), samples as usize, queries as usize))
    }

    /// A new monitor; returns its handle (≥ 0) or a negative status.
    #[unsafe(no_mangle)]
    pub extern "C" fn fm_mon_new(
        nugget: f64,
        sill: f64,
        range_m: f64,
        neighbors: u32,
        alpha: f64,
    ) -> i32 {
        let model = spatial::Model {
            nugget,
            sill,
            range_m,
            neighbors: neighbors as usize,
            alpha,
        };
        let mut handle = 0;
        let status = run(|| {
            handle = spatial::monitor_new(model)?;
            Ok(super::Output::default())
        });
        if status == 0 { handle } else { status }
    }

    /// Bets on one observation. Output f64: see [`spatial::monitor_observe`].
    #[unsafe(no_mangle)]
    pub extern "C" fn fm_mon_observe(
        h: u32,
        x: f64,
        y: f64,
        z: f64,
        prior: f64,
        value: f64,
    ) -> i32 {
        run(|| spatial::monitor_observe(h as usize, x, y, z, prior, value))
    }

    /// Input: `n` × (x, y, z, prior, value) conditioning samples, added to the history without a bet.
    #[unsafe(no_mangle)]
    pub extern "C" fn fm_mon_condition(h: u32, n: u32) -> i32 {
        run(|| spatial::monitor_condition(h as usize, &take_input(), n as usize))
    }

    /// Input: `n` × (x, y, z, prior) queries against the monitor's history. Output: as [`spatial::estimate`].
    #[unsafe(no_mangle)]
    pub extern "C" fn fm_mon_estimate(h: u32, n: u32) -> i32 {
        run(|| spatial::monitor_estimate(h as usize, &take_input(), n as usize))
    }

    /// Forgets the history and the e-processes, keeping the model.
    #[unsafe(no_mangle)]
    pub extern "C" fn fm_mon_reset(h: u32) -> i32 {
        run(|| spatial::monitor_reset(h as usize))
    }

    /// Frees the handle.
    #[unsafe(no_mangle)]
    pub extern "C" fn fm_mon_free(h: u32) -> i32 {
        run(|| spatial::monitor_free(h as usize))
    }

    // --- fmine-plant, fmine-numerics ---

    /// Input: `n` work indices, then `n` weights (all zero: equal). Output: see [`extras::grind`].
    #[unsafe(no_mangle)]
    pub extern "C" fn fm_grind(
        n: u32,
        f80_um: f64,
        p80_um: f64,
        power_kw: f64,
        utilization: f64,
    ) -> i32 {
        run(|| {
            extras::grind(
                &take_input(),
                n as usize,
                f80_um,
                p80_um,
                power_kw,
                utilization,
            )
        })
    }

    /// Input: `states` priors, `states × actions` payoffs (row per state), `states × signals` likelihoods.
    #[unsafe(no_mangle)]
    pub extern "C" fn fm_voi(states: u32, actions: u32, signals: u32, cost: f64) -> i32 {
        run(|| {
            extras::voi(
                &take_input(),
                states as usize,
                actions as usize,
                signals as usize,
                cost,
            )
        })
    }

    /// Input: the losses.
    #[unsafe(no_mangle)]
    pub extern "C" fn fm_cvar(alpha: f64) -> i32 {
        run(|| extras::cvar(&take_input(), alpha))
    }
}
