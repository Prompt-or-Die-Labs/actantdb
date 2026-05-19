//! Concrete [`crate::Destination`] implementations.
//!
//! The filesystem destination is always available; S3 / GCS / Azure / IPFS
//! are feature-gated so the lean install does not pull cloud SDKs.

mod fs;

#[cfg(feature = "s3")]
mod s3;

#[cfg(feature = "gcs")]
mod gcs;

#[cfg(feature = "azure")]
mod azure;

#[cfg(feature = "ipfs")]
mod ipfs;

pub use fs::FilesystemDestination;

#[cfg(feature = "s3")]
pub use s3::S3Destination;

#[cfg(feature = "gcs")]
pub use gcs::{GcsConfig, GcsDestination};

#[cfg(feature = "azure")]
pub use azure::{AzureConfig, AzureDestination};

#[cfg(feature = "ipfs")]
pub use ipfs::IpfsDestination;
