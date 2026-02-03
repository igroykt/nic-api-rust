use thiserror::Error;

/// Errors that can occur when working with the NIC.RU DNS API
#[derive(Error, Debug)]
pub enum DnsApiError {
    /// OAuth token has expired and needs to be refreshed
    #[error("OAuth token has expired")]
    ExpiredToken,

    /// Invalid DNS record data was provided
    #[error("Invalid record data: {0}")]
    InvalidRecord(String),

    /// The specified service was not found
    #[error("Service not found: {0}")]
    ServiceNotFound(String),

    /// The specified DNS zone was not found
    #[error("Zone not found: {0}")]
    ZoneNotFound(String),

    /// The DNS zone already exists
    #[error("Zone already exists: {0}")]
    ZoneAlreadyExists(String),

    /// Invalid domain name was provided
    #[error("Invalid domain name: {0}")]
    InvalidDomainName(String),

    /// HTTP request error
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    /// XML parsing error
    #[error("XML parsing error: {0}")]
    XmlError(String),

    /// OAuth2 authentication error
    #[error("OAuth2 error: {0}")]
    OAuth2Error(String),

    /// Generic API error
    #[error("API error: {0}")]
    ApiError(String),

    /// Invalid TTL value
    #[error("Invalid TTL value")]
    InvalidTtl,

    /// Invalid record ID
    #[error("Invalid record ID")]
    InvalidRecordId,
}

impl From<quick_xml::Error> for DnsApiError {
    fn from(err: quick_xml::Error) -> Self {
        DnsApiError::XmlError(err.to_string())
    }
}

impl From<quick_xml::DeError> for DnsApiError {
    fn from(err: quick_xml::DeError) -> Self {
        DnsApiError::XmlError(err.to_string())
    }
}

impl From<quick_xml::events::attributes::AttrError> for DnsApiError {
    fn from(err: quick_xml::events::attributes::AttrError) -> Self {
        DnsApiError::XmlError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, DnsApiError>;
