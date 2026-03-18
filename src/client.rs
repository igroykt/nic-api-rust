use quick_xml::Reader;
use quick_xml::events::Event;
use reqwest::Client;

use crate::auth::{Token, TokenManager};
use crate::error::{DnsApiError, Result};
use crate::models::{
    AaaaRecord, ARecord, CnameRecord, DnameRecord, DnsRecord, MxRecord, NicService, NicZone,
    NsRecord, PtrRecord, SrvRecord, TxtRecord,
};

/// Main API client for interacting with NIC.RU DNS API
pub struct DnsApi {
    client: Client,
    base_url: String,
    token_manager: TokenManager,
    pub default_service: Option<String>,
    pub default_zone: Option<String>,
}

impl DnsApi {
    /// Create a new DnsApi instance
    ///
    /// # Arguments
    ///
    /// * `app_login` - OAuth application login
    /// * `app_password` - OAuth application password
    /// * `token` - Optional existing OAuth token
    /// * `offline` - Token lifetime in seconds (default: 3600)
    /// * `scope` - OAuth scope (default: ".+:/dns-master/.+")
    pub fn new(
        app_login: impl Into<String>,
        app_password: impl Into<String>,
        token: Option<Token>,
        offline: Option<u64>,
        scope: Option<String>,
    ) -> Self {
        let base_url = "https://api.nic.ru".to_string();
        let offline = offline.unwrap_or(3600);
        let scope = scope.unwrap_or_else(|| ".+:/dns-master/.+".to_string());

        let mut token_manager = TokenManager::new(
            app_login,
            app_password,
            base_url.clone(),
            offline,
            scope,
        );

        if let Some(token) = token {
            token_manager.set_token(token);
        }

        Self {
            client: Client::new(),
            base_url,
            token_manager,
            default_service: None,
            default_zone: None,
        }
    }

    /// Get OAuth token using username and password
    pub async fn get_token(
        &mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Result<Token> {
        self.token_manager
            .get_token_with_password(username, password)
            .await
    }

    /// Refresh OAuth token
    pub async fn refresh_token(&mut self, refresh_token: impl Into<String>) -> Result<Token> {
        self.token_manager.refresh_token(refresh_token).await
    }

    /// Build full API URL
    fn url_for(&self, path: &str) -> String {
        format!("{}/dns-master/{}", self.base_url, path)
    }

    /// Make authenticated GET request
    async fn get(&self, path: &str) -> Result<String> {
        let token = self
            .token_manager
            .access_token()
            .ok_or(DnsApiError::OAuth2Error(
                "No access token available".to_string(),
            ))?;

        let url = self.url_for(path);
        log::debug!("GET {}", url);

        let response = self
            .client
            .get(url)
            .bearer_auth(token)
            .send()
            .await?;

        log::debug!("Response status: {}", response.status());
        let text = response.text().await?;
        log::debug!("Response body: {}", text);
        self.check_response(&text)?;
        Ok(text)
    }

    /// Make authenticated POST request
    async fn post(&self, path: &str, data: Option<&str>) -> Result<String> {
        let token = self
            .token_manager
            .access_token()
            .ok_or(DnsApiError::OAuth2Error(
                "No access token available".to_string(),
            ))?;

        let url = self.url_for(path);
        log::debug!("POST {}", url);

        let mut request = self.client.post(url).bearer_auth(token);

        if let Some(data) = data {
            log::debug!("Request body: {}", data);
            request = request
                .header("Content-Type", "application/xml")
                .body(data.to_string());
        }

        let response = request.send().await?;
        log::debug!("Response status: {}", response.status());
        let text = response.text().await?;
        log::debug!("Response body: {}", text);
        self.check_response(&text)?;
        Ok(text)
    }

    /// Make authenticated PUT request
    async fn put(&self, path: &str, data: Option<&str>) -> Result<String> {
        let token = self
            .token_manager
            .access_token()
            .ok_or(DnsApiError::OAuth2Error(
                "No access token available".to_string(),
            ))?;

        let url = self.url_for(path);
        log::debug!("PUT {}", url);

        let mut request = self.client.put(url).bearer_auth(token);

        if let Some(data) = data {
            log::debug!("Request body: {}", data);
            request = request
                .header("Content-Type", "application/xml")
                .body(data.to_string());
        }

        let response = request.send().await?;
        log::debug!("Response status: {}", response.status());
        let text = response.text().await?;
        log::debug!("Response body: {}", text);
        self.check_response(&text)?;
        Ok(text)
    }

    /// Make authenticated DELETE request
    async fn delete(&self, path: &str) -> Result<String> {
        let token = self
            .token_manager
            .access_token()
            .ok_or(DnsApiError::OAuth2Error(
                "No access token available".to_string(),
            ))?;

        let url = self.url_for(path);
        log::debug!("DELETE {}", url);

        let response = self
            .client
            .delete(url)
            .bearer_auth(token)
            .send()
            .await?;

        log::debug!("Response status: {}", response.status());
        let text = response.text().await?;
        log::debug!("Response body: {}", text);
        self.check_response(&text)?;
        Ok(text)
    }

    /// Check API response for errors
    fn check_response(&self, xml: &str) -> Result<()> {
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let mut buf = Vec::new();
        let mut status = String::new();
        let mut error_code: Option<i32> = None;
        let mut error_text = String::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => match e.name().as_ref() {
                    b"status" => {
                        if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) {
                            status = t.unescape().unwrap().to_string();
                        }
                    }
                    b"error" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"code" {
                                if let Ok(value) = attr.unescape_value() {
                                    error_code = value.parse().ok();
                                }
                            }
                        }
                        if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) {
                            error_text = t.unescape().unwrap().to_string();
                        }
                    }
                    _ => {}
                },
                Ok(Event::Eof) => break,
                Err(e) => return Err(DnsApiError::XmlError(e.to_string())),
                _ => {}
            }
            buf.clear();
        }

        if status != "success" {
            return match error_code {
                Some(4097) => Err(DnsApiError::ExpiredToken),
                Some(4327) => Err(DnsApiError::InvalidRecord(error_text)),
                Some(4009) => Err(DnsApiError::ServiceNotFound(error_text)),
                Some(4028) => Err(DnsApiError::ZoneNotFound(error_text)),
                Some(4020) => Err(DnsApiError::InvalidDomainName(error_text)),
                Some(4021) => Err(DnsApiError::ZoneAlreadyExists(error_text)),
                _ => Err(DnsApiError::ApiError(error_text)),
            };
        }

        Ok(())
    }

    /// Extract data element from XML response
    fn extract_data(&self, xml: &str) -> Result<String> {
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let mut buf = Vec::new();
        let mut in_data = false;
        let mut data_content = String::new();
        let mut depth = 0;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) if e.name().as_ref() == b"data" && !in_data => {
                    in_data = true;
                    depth = 1;
                }
                Ok(Event::Start(e)) if in_data => {
                    depth += 1;
                    data_content.push_str(&format!("<{}>", String::from_utf8_lossy(e.name().as_ref())));
                }
                Ok(Event::End(e)) if in_data => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    data_content.push_str(&format!("</{}>", String::from_utf8_lossy(e.name().as_ref())));
                }
                Ok(Event::Text(e)) if in_data => {
                    data_content.push_str(&e.unescape().unwrap());
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(DnsApiError::XmlError(e.to_string())),
                _ => {}
            }
            buf.clear();
        }

        Ok(data_content)
    }

    /// Get list of available services
    pub async fn services(&self) -> Result<Vec<NicService>> {
        let response = self.get("services").await?;
        let data = self.extract_data(&response)?;
        
        let mut services = Vec::new();
        let mut reader = Reader::from_str(&data);
        reader.trim_text(true);
        
        let mut buf = Vec::new();
        #[allow(unused_assignments)]
        let mut current_element = String::new();
        
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name().as_ref() == b"service" => {
                    // Capture the entire service element
                    current_element = format!("<{}", String::from_utf8_lossy(e.name().as_ref()));
                    for attr in e.attributes().flatten() {
                        current_element.push_str(&format!(
                            " {}=\"{}\"",
                            String::from_utf8_lossy(attr.key.as_ref()),
                            attr.unescape_value().unwrap()
                        ));
                    }
                    current_element.push_str(" />");
                    
                    if let Ok(service) = NicService::from_xml(&current_element) {
                        services.push(service);
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(DnsApiError::XmlError(e.to_string())),
                _ => {}
            }
            buf.clear();
        }
        
        Ok(services)
    }

    /// Get DNS zones for a service
    pub async fn zones(&self, service: Option<&str>) -> Result<Vec<NicZone>> {
        let service = service
            .or(self.default_service.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;

        let path = format!("services/{}/zones", service);
        let response = self.get(&path).await?;

        let mut zones = Vec::new();
        let mut reader = Reader::from_str(&response);
        reader.trim_text(true);

        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Empty(e)) | Ok(Event::Start(e)) if e.name().as_ref() == b"zone" => {
                    let mut admin = String::new();
                    let mut enable = false;
                    let mut has_changes = false;
                    let mut has_primary = false;
                    let mut id = 0u64;
                    let mut idn_name = String::new();
                    let mut name = String::new();
                    let mut payer = String::new();
                    let mut service_name = String::new();

                    for attr in e.attributes() {
                        let attr = attr.map_err(|e| DnsApiError::XmlError(e.to_string()))?;
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                        let value = attr.unescape_value().unwrap_or_default();

                        match key {
                            "admin" => admin = value.to_string(),
                            "enable" => enable = value == "true",
                            "has-changes" => has_changes = value == "true",
                            "has-primary" => has_primary = value == "true",
                            "id" => id = value.parse().unwrap_or(0),
                            "idn-name" => idn_name = value.to_string(),
                            "name" => name = value.to_string(),
                            "payer" => payer = value.to_string(),
                            "service" => service_name = value.to_string(),
                            _ => {}
                        }
                    }

                    zones.push(NicZone {
                        admin,
                        enable,
                        has_changes,
                        has_primary,
                        id,
                        idn_name,
                        name,
                        payer,
                        service: service_name,
                    });
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(DnsApiError::XmlError(e.to_string())),
                _ => {}
            }
            buf.clear();
        }

        Ok(zones)
    }

    /// Commit changes to a DNS zone
    pub async fn commit(&self, service: Option<&str>, zone: Option<&str>) -> Result<()> {
        let service = service
            .or(self.default_service.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        
        let zone = zone
            .or(self.default_zone.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No zone specified".to_string()))?;

        let path = format!("services/{}/zones/{}/commit", service, zone);
        self.post(&path, None).await?;
        Ok(())
    }

    /// Get DNS records for a zone
    pub async fn records(&self, service: Option<&str>, zone: Option<&str>) -> Result<Vec<DnsRecord>> {
        let service = service
            .or(self.default_service.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        
        let zone = zone
            .or(self.default_zone.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No zone specified".to_string()))?;

        let path = format!("services/{}/zones/{}/records", service, zone);
        let response = self.get(&path).await?;

        let mut records = Vec::new();
        let mut reader = Reader::from_str(&response);
        reader.trim_text(true);

        let mut buf = Vec::new();

        // State for current <rr>
        let mut in_rr = false;
        let mut rr_id: Option<u64> = None;
        let mut rr_name = String::new();
        let mut rr_ttl: Option<u32> = None;
        let mut rr_type = String::new();
        // Simple type-specific fields
        let mut rr_a = String::new();
        let mut rr_aaaa = String::new();
        let mut rr_cname = String::new();
        let mut rr_ns = String::new();
        let mut rr_ptr = String::new();
        let mut rr_dname = String::new();
        // TXT
        let mut in_txt_string = false;
        let mut rr_txt = String::new();
        // MX
        let mut in_mx = false;
        let mut rr_mx_preference: u16 = 0;
        let mut rr_mx_exchange = String::new();
        let mut in_mx_preference = false;
        let mut in_mx_exchange = false;
        // SRV
        let mut in_srv = false;
        let mut rr_srv_priority: u16 = 0;
        let mut rr_srv_weight: u16 = 0;
        let mut rr_srv_port: u16 = 0;
        let mut rr_srv_target = String::new();
        let mut in_srv_priority = false;
        let mut in_srv_weight = false;
        let mut in_srv_port = false;
        let mut in_srv_target = false;
        // Current simple text element tag
        let mut current_tag = String::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    match tag.as_str() {
                        "rr" => {
                            in_rr = true;
                            rr_id = None;
                            rr_name.clear();
                            rr_ttl = None;
                            rr_type.clear();
                            rr_a.clear();
                            rr_aaaa.clear();
                            rr_cname.clear();
                            rr_ns.clear();
                            rr_ptr.clear();
                            rr_dname.clear();
                            rr_txt.clear();
                            rr_mx_preference = 0;
                            rr_mx_exchange.clear();
                            rr_srv_priority = 0;
                            rr_srv_weight = 0;
                            rr_srv_port = 0;
                            rr_srv_target.clear();
                            in_mx = false;
                            in_srv = false;
                            // Parse id attribute if present
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"id" {
                                    rr_id = attr.unescape_value().ok()
                                        .and_then(|v| v.parse().ok());
                                }
                            }
                        }
                        "mx" if in_rr => { in_mx = true; }
                        "srv" if in_rr => { in_srv = true; }
                        "string" if in_rr && rr_type == "TXT" => { in_txt_string = true; }
                        "preference" if in_mx => { in_mx_preference = true; }
                        "exchange" if in_mx => { in_mx_exchange = true; }
                        "priority" if in_srv => { in_srv_priority = true; }
                        "weight" if in_srv => { in_srv_weight = true; }
                        "port" if in_srv => { in_srv_port = true; }
                        "target" if in_srv => { in_srv_target = true; }
                        other if in_rr => { current_tag = other.to_string(); }
                        _ => {}
                    }
                }
                Ok(Event::End(ref e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    match tag.as_str() {
                        "rr" if in_rr => {
                            in_rr = false;
                            let record = match rr_type.as_str() {
                                "A" => Some(DnsRecord::A(ARecord {
                                    id: rr_id,
                                    name: rr_name.clone(),
                                    idn_name: None,
                                    ttl: rr_ttl,
                                    a: rr_a.clone(),
                                })),
                                "AAAA" => Some(DnsRecord::AAAA(AaaaRecord {
                                    id: rr_id,
                                    name: rr_name.clone(),
                                    idn_name: None,
                                    ttl: rr_ttl,
                                    aaaa: rr_aaaa.clone(),
                                })),
                                "CNAME" => Some(DnsRecord::CNAME(CnameRecord {
                                    id: rr_id,
                                    name: rr_name.clone(),
                                    idn_name: None,
                                    ttl: rr_ttl,
                                    cname: rr_cname.clone(),
                                })),
                                "NS" => Some(DnsRecord::NS(NsRecord {
                                    id: rr_id,
                                    name: rr_name.clone(),
                                    idn_name: None,
                                    ns: rr_ns.clone(),
                                })),
                                "TXT" => Some(DnsRecord::TXT(TxtRecord {
                                    id: rr_id,
                                    name: rr_name.clone(),
                                    idn_name: None,
                                    ttl: rr_ttl,
                                    txt: rr_txt.clone(),
                                })),
                                "PTR" => Some(DnsRecord::PTR(PtrRecord {
                                    id: rr_id,
                                    name: rr_name.clone(),
                                    idn_name: None,
                                    ttl: rr_ttl,
                                    ptr: rr_ptr.clone(),
                                })),
                                "DNAME" => Some(DnsRecord::DNAME(DnameRecord {
                                    id: rr_id,
                                    name: rr_name.clone(),
                                    idn_name: None,
                                    ttl: rr_ttl,
                                    dname: rr_dname.clone(),
                                })),
                                "MX" => Some(DnsRecord::MX(MxRecord {
                                    id: rr_id,
                                    name: rr_name.clone(),
                                    idn_name: None,
                                    ttl: rr_ttl,
                                    preference: rr_mx_preference,
                                    exchange: rr_mx_exchange.clone(),
                                })),
                                "SRV" => Some(DnsRecord::SRV(SrvRecord {
                                    id: rr_id,
                                    name: rr_name.clone(),
                                    idn_name: None,
                                    ttl: rr_ttl,
                                    priority: rr_srv_priority,
                                    weight: rr_srv_weight,
                                    port: rr_srv_port,
                                    target: rr_srv_target.clone(),
                                })),
                                _ => None, // Unknown record type; skip
                            };
                            if let Some(r) = record {
                                records.push(r);
                            }
                        }
                        "mx" => { in_mx = false; }
                        "srv" => { in_srv = false; }
                        "string" => { in_txt_string = false; }
                        "preference" => { in_mx_preference = false; }
                        "exchange" => { in_mx_exchange = false; }
                        "priority" => { in_srv_priority = false; }
                        "weight" => { in_srv_weight = false; }
                        "port" => { in_srv_port = false; }
                        "target" => { in_srv_target = false; }
                        _ => { current_tag.clear(); }
                    }
                }
                Ok(Event::Text(ref e)) if in_rr => {
                    let text = e.unescape().map_err(|e| DnsApiError::XmlError(e.to_string()))?.to_string();
                    if in_txt_string {
                        rr_txt = text;
                    } else if in_mx_preference {
                        rr_mx_preference = text.parse().unwrap_or(0);
                    } else if in_mx_exchange {
                        rr_mx_exchange = text;
                    } else if in_srv_priority {
                        rr_srv_priority = text.parse().unwrap_or(0);
                    } else if in_srv_weight {
                        rr_srv_weight = text.parse().unwrap_or(0);
                    } else if in_srv_port {
                        rr_srv_port = text.parse().unwrap_or(0);
                    } else if in_srv_target {
                        rr_srv_target = text;
                    } else {
                        match current_tag.as_str() {
                            "name" => rr_name = text,
                            "ttl" => rr_ttl = text.parse().ok(),
                            "type" => rr_type = text,
                            "a" => rr_a = text,
                            "aaaa" => rr_aaaa = text,
                            "cname" => rr_cname = text,
                            "ns" => rr_ns = text,
                            "ptr" => rr_ptr = text,
                            "dname" => rr_dname = text,
                            _ => {}
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(DnsApiError::XmlError(e.to_string())),
                _ => {}
            }
            buf.clear();
        }

        Ok(records)
    }

    /// Add DNS record(s) to a zone
    pub async fn add_record(
        &self,
        records: Vec<DnsRecord>,
        service: Option<&str>,
        zone: Option<&str>,
    ) -> Result<Vec<DnsRecord>> {
        let service = service
            .or(self.default_service.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        
        let zone = zone
            .or(self.default_zone.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No zone specified".to_string()))?;

        // Build XML request
        let mut rr_list = String::new();
        for record in &records {
            rr_list.push_str(&record.to_xml()?);
        }

        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" ?><request><rr-list>{}</rr-list></request>"#,
            rr_list
        );

        let path = format!("services/{}/zones/{}/records", service, zone);
        let _response = self.put(&path, Some(&xml)).await?;
        
        // Parse response and return created records
        // Simplified - full implementation would parse XML
        Ok(Vec::new())
    }

    /// Delete a DNS record by ID
    pub async fn delete_record(
        &self,
        record_id: u64,
        service: Option<&str>,
        zone: Option<&str>,
    ) -> Result<()> {
        let service = service
            .or(self.default_service.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        
        let zone = zone
            .or(self.default_zone.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No zone specified".to_string()))?;

        let path = format!("services/{}/zones/{}/records/{}", service, zone, record_id);
        self.delete(&path).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_api() -> DnsApi {
        let token = Token {
            access_token: "test_token".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            refresh_token: Some("refresh".to_string()),
            scope: Some("test".to_string()),
        };
        DnsApi::new("app_login", "app_password", Some(token), None, None)
    }

    #[test]
    fn dns_api_new_default_params() {
        let api = create_test_api();
        assert_eq!(api.base_url, "https://api.nic.ru");
        assert_eq!(api.default_service, None);
        assert_eq!(api.default_zone, None);
        assert_eq!(api.token_manager.access_token(), Some("test_token"));
    }

    #[test]
    fn dns_api_new_custom_params() {
        let api = DnsApi::new("login", "pass", None, Some(7200), Some("custom_scope".to_string()));
        assert_eq!(api.base_url, "https://api.nic.ru");
        assert_eq!(api.token_manager.access_token(), None);
    }

    #[test]
    fn url_for_constructs_correct_path() {
        let api = create_test_api();
        assert_eq!(api.url_for("services"), "https://api.nic.ru/dns-master/services");
        assert_eq!(
            api.url_for("services/mysvc/zones/myzone/records"),
            "https://api.nic.ru/dns-master/services/mysvc/zones/myzone/records"
        );
    }

    #[test]
    fn check_response_success() {
        let api = create_test_api();
        let xml = "<response><status>success</status></response>";
        assert!(api.check_response(xml).is_ok());
    }

    #[test]
    fn check_response_expired_token() {
        let api = create_test_api();
        let xml = r#"<response><status>error</status><errors><error code="4097">Token expired</error></errors></response>"#;
        let err = api.check_response(xml).unwrap_err();
        assert!(matches!(err, DnsApiError::ExpiredToken));
    }

    #[test]
    fn check_response_invalid_record() {
        let api = create_test_api();
        let xml = r#"<response><status>error</status><errors><error code="4327">bad record</error></errors></response>"#;
        let err = api.check_response(xml).unwrap_err();
        assert!(matches!(err, DnsApiError::InvalidRecord(_)));
    }

    #[test]
    fn check_response_service_not_found() {
        let api = create_test_api();
        let xml = r#"<response><status>error</status><errors><error code="4009">no service</error></errors></response>"#;
        let err = api.check_response(xml).unwrap_err();
        assert!(matches!(err, DnsApiError::ServiceNotFound(_)));
    }

    #[test]
    fn check_response_zone_not_found() {
        let api = create_test_api();
        let xml = r#"<response><status>error</status><errors><error code="4028">no zone</error></errors></response>"#;
        let err = api.check_response(xml).unwrap_err();
        assert!(matches!(err, DnsApiError::ZoneNotFound(_)));
    }

    #[test]
    fn check_response_invalid_domain_name() {
        let api = create_test_api();
        let xml = r#"<response><status>error</status><errors><error code="4020">bad domain</error></errors></response>"#;
        let err = api.check_response(xml).unwrap_err();
        assert!(matches!(err, DnsApiError::InvalidDomainName(_)));
    }

    #[test]
    fn check_response_zone_already_exists() {
        let api = create_test_api();
        let xml = r#"<response><status>error</status><errors><error code="4021">zone exists</error></errors></response>"#;
        let err = api.check_response(xml).unwrap_err();
        assert!(matches!(err, DnsApiError::ZoneAlreadyExists(_)));
    }

    #[test]
    fn check_response_unknown_error() {
        let api = create_test_api();
        let xml = r#"<response><status>error</status><errors><error code="9999">something</error></errors></response>"#;
        let err = api.check_response(xml).unwrap_err();
        assert!(matches!(err, DnsApiError::ApiError(_)));
    }

    #[test]
    fn extract_data_with_content() {
        let api = create_test_api();
        // extract_data captures nested Start/End events and text, not Empty events
        let xml = "<response><status>success</status><data><zone><name>example.com</name></zone></data></response>";
        let data = api.extract_data(xml).unwrap();
        assert!(data.contains("<zone>"));
        assert!(data.contains("example.com"));
    }

    #[test]
    fn extract_data_empty() {
        let api = create_test_api();
        let xml = "<response><status>success</status><data></data></response>";
        let data = api.extract_data(xml).unwrap();
        assert_eq!(data, "");
    }

    #[test]
    fn extract_data_no_data_element() {
        let api = create_test_api();
        let xml = "<response><status>success</status></response>";
        let data = api.extract_data(xml).unwrap();
        assert_eq!(data, "");
    }
}
