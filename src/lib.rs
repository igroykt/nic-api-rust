//! # nic-api-rust
//!
//! Rust library for managing DNS zones and records via the NIC.RU API.
//!
//! This library provides an asynchronous interface for interacting with the NIC.RU DNS
//! management API, supporting OAuth2 authentication and full CRUD operations for DNS records.
//!
//! ## Features
//!
//! - OAuth2 Resource Owner Password Credentials Grant flow
//! - Token refresh support
//! - Asynchronous API using tokio and reqwest
//! - Support for 13 DNS record types (A, AAAA, CNAME, MX, NS, TXT, SOA, SRV, PTR, DNAME, HINFO, NAPTR, RP)
//! - Service and zone management
//! - Commit-based workflow for applying changes
//!
//! ## Example
//!
//! ```no_run
//! use nic_api_rust::{DnsApi, models::ARecord};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Initialize API client
//!     let mut api = DnsApi::new(
//!         "your_app_login",
//!         "your_app_password",
//!         None,  // No existing token
//!         None,  // Use default offline duration
//!         None,  // Use default scope
//!         None,  // No default service
//!         None,  // No default zone
//!     );
//!
//!     // Authenticate
//!     api.get_token("your_username", "your_password").await?;
//!
//!     // Set default service and zone
//!     api.service_id = Some("MY_SERVICE".to_string());
//!     api.zone = Some("example.com".to_string());
//!
//!     // Get services
//!     let services = api.services().await?;
//!     println!("Available services: {:?}", services);
//!
//!     Ok(())
//! }
//! ```

pub mod auth;
pub mod client;
pub mod error;
pub mod models;

// Re-export main types
pub use auth::{Token, TokenManager};
pub use client::DnsApi;
pub use error::{DnsApiError, Result};
pub use models::{
    ARecord, AaaaRecord, CnameRecord, DnameRecord, DnsRecord, HinfoRecord, MxRecord, 
    NaptrRecord, NicService, NicZone, NicZoneRevision, NsRecord, PtrRecord, RpRecord, 
    SoaRecord, SrvRecord, TxtRecord,
};
