//! Time helpers — every timestamp in the substrate is RFC3339 UTC.

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// Current time as an RFC3339 UTC string.
pub fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("rfc3339 formatting must succeed")
}
