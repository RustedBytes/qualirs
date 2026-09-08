// EXPECT: God Module
// EXPECT: Public API Explosion
// EXPECT: Feature Concentration
// EXPECT: Cyclic Crate Dependency
// EXPECT: Layer Violation
// EXPECT: Unstable Dependency
// EXPECT: Leaky Error Abstraction
// EXPECT: Hidden Global State
// EXPECT: Public API Leak
// EXPECT: Feature Flag Sprawl
// EXPECT: Circular Module Dependency
// EXPECT: Test-only Dependency in Production
// EXPECT: Duplicate Dependency Versions

#![allow(dead_code, unused_imports)]

pub mod domain;
pub mod private;
pub mod unstable;

// Explicit crate-relative roots support Q0004's exploratory coupling hint.
use crate::{domain::D, private::P, unstable::U, a::A, b::B, c::C};

use alpha::A;
use beta::B;
use gamma::C;
use delta::D;
use epsilon::E;
use zeta::F;
use eta::G;
use theta::H;
use iota::I;
use kappa::J;
use lambda::K;
use mu::L;
use nu::M;
use xi::N;
use omicron::O;
use pi::P;
use rho::Q;
use internal_sdk::raw::PrivateClient;
use pretty_assertions::assert_eq;

#[cfg(feature = "a")]
pub fn feature_a() {}
#[cfg(feature = "b")]
pub fn feature_b() {}
#[cfg(feature = "c")]
pub fn feature_c() {}
#[cfg(feature = "d")]
pub fn feature_d() {}
#[cfg(feature = "e")]
pub fn feature_e() {}
#[cfg(feature = "f")]
pub fn feature_f() {}
#[cfg(feature = "g")]
pub fn feature_g() {}
#[cfg(feature = "h")]
pub fn feature_h() {}
#[cfg(feature = "i")]
pub fn feature_i() {}

static CACHE_A: std::sync::OnceLock<String> = std::sync::OnceLock::new();
static CACHE_B: std::sync::OnceLock<String> = std::sync::OnceLock::new();
static CACHE_C: std::sync::OnceLock<String> = std::sync::OnceLock::new();
static CACHE_D: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub enum AppError {
    Database(sqlx::Error),
}

pub fn expose_database_row() -> sqlx::Row {
    unimplemented!()
}

pub fn use_dev_dependency() {
    assert_eq!(1, 1);
}

pub fn domain_depends_on_infrastructure() {
    use crate::domain::service::OrderService;
    use crate::infrastructure::postgres::OrderRepository;
}

pub fn module_cycle_marker() {
    use crate::domain::infrastructure::Adapter;
    use crate::infrastructure::domain::Entity;
}

pub fn helper_00() {}
pub fn helper_01() {}
pub fn helper_02() {}
pub fn helper_03() {}
pub fn helper_04() {}
pub fn helper_05() {}
pub fn helper_06() {}
pub fn helper_07() {}
pub fn helper_08() {}
pub fn helper_09() {}
pub fn helper_10() {}
pub fn helper_11() {}
pub fn helper_12() {}
pub fn helper_13() {}
pub fn helper_14() {}
pub fn helper_15() {}
pub fn helper_16() {}
pub fn helper_17() {}
pub fn helper_18() {}
pub fn helper_19() {}
pub fn helper_20() {}
pub fn helper_21() {}
pub fn helper_22() {}
pub fn helper_23() {}
pub fn helper_24() {}
