//! DESIGN: Crucible's first plant slice accounts for conserved elemental mass
//! across a declared boundary and interval (plan §§17.3, 17.5; BD-175/177/313).
//! It carries source claims and physical inventory; it does not fit recovery.
//! Determinism: Exact

pub mod cards;
pub mod flowsheet;
pub mod grinding;
pub mod inventory;
pub mod measurements;
pub mod production;
pub mod raw_measurements;
pub mod reconciliation;
pub mod rtd;
pub mod trajectory;
