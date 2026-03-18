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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_expired_token() {
        assert_eq!(DnsApiError::ExpiredToken.to_string(), "OAuth token has expired");
    }

    #[test]
    fn display_invalid_record() {
        assert_eq!(
            DnsApiError::InvalidRecord("bad data".to_string()).to_string(),
            "Invalid record data: bad data"
        );
    }

    #[test]
    fn display_service_not_found() {
        assert_eq!(
            DnsApiError::ServiceNotFound("svc".to_string()).to_string(),
            "Service not found: svc"
        );
    }

    #[test]
    fn display_zone_not_found() {
        assert_eq!(
            DnsApiError::ZoneNotFound("zone".to_string()).to_string(),
            "Zone not found: zone"
        );
    }

    #[test]
    fn display_zone_already_exists() {
        assert_eq!(
            DnsApiError::ZoneAlreadyExists("zone".to_string()).to_string(),
            "Zone already exists: zone"
        );
    }

    #[test]
    fn display_invalid_domain_name() {
        assert_eq!(
            DnsApiError::InvalidDomainName("bad.domain".to_string()).to_string(),
            "Invalid domain name: bad.domain"
        );
    }

    #[test]
    fn display_xml_error() {
        assert_eq!(
            DnsApiError::XmlError("parse error".to_string()).to_string(),
            "XML parsing error: parse error"
        );
    }

    #[test]
    fn display_oauth2_error() {
        assert_eq!(
            DnsApiError::OAuth2Error("auth failed".to_string()).to_string(),
            "OAuth2 error: auth failed"
        );
    }

    #[test]
    fn display_api_error() {
        assert_eq!(
            DnsApiError::ApiError("something bad".to_string()).to_string(),
            "API error: something bad"
        );
    }

    #[test]
    fn display_invalid_ttl() {
        assert_eq!(DnsApiError::InvalidTtl.to_string(), "Invalid TTL value");
    }

    #[test]
    fn display_invalid_record_id() {
        assert_eq!(DnsApiError::InvalidRecordId.to_string(), "Invalid record ID");
    }

    #[test]
    fn from_quick_xml_error() {
        let xml_err = quick_xml::Error::UnexpectedEof("test".to_string());
        let err: DnsApiError = xml_err.into();
        assert!(matches!(err, DnsApiError::XmlError(_)));
    }

    #[test]
    fn from_quick_xml_de_error() {
        let de_err = quick_xml::DeError::UnexpectedEof;
        let err: DnsApiError = de_err.into();
        assert!(matches!(err, DnsApiError::XmlError(_)));
    }
}
