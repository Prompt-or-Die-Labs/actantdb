//! Compile-time parity check between [`EmulatorDriver`] and [`cdp::CdpDriver`].
//!
//! `check_parity` is generic over any `Driver`; we call it once with
//! `EmulatorDriver` unconditionally and once with `CdpDriver` only when the
//! `cdp` feature is enabled. If either type stops implementing the same
//! `Driver` trait, this test fails to compile - which is the point.

use actant_worker_browser::{Driver, EmulatorDriver};

fn check_parity<D: Driver>(_d: &D) {}

#[test]
fn emulator_implements_driver() {
    let d = EmulatorDriver::new("parity");
    check_parity(&d);
}

#[cfg(feature = "cdp")]
mod cdp_parity {
    use super::check_parity;
    use actant_worker_browser::cdp::CdpDriver;

    // Compile-only: assert `fn(&CdpDriver) -> ()` typechecks. No runtime
    // launch (would require Chrome on PATH).
    #[allow(dead_code)]
    fn _types_match() {
        fn assert_driver<D: actant_worker_browser::Driver>() {}
        assert_driver::<CdpDriver>();
        // Reference `check_parity` so the function-item type is exercised
        // against `CdpDriver` exactly the same way it is against the
        // emulator.
        let _f: fn(&CdpDriver) = check_parity::<CdpDriver>;
    }
}
