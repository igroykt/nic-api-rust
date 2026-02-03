use quick_xml::Reader;
use quick_xml::events::Event;
use reqwest::Client;

use crate::auth::{Token, TokenManager};
use crate::error::{DnsApiError, Result};
use crate::models::{DnsRecord, NicService, NicZone};

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

        let response = self
            .client
            .get(self.url_for(path))
            .bearer_auth(token)
            .send()
            .await?;

        let text = response.text().await?;
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

        let mut request = self.client.post(self.url_for(path)).bearer_auth(token);

        if let Some(data) = data {
            request = request
                .header("Content-Type", "application/xml")
                .body(data.to_string());
        }

        let response = request.send().await?;
        let text = response.text().await?;
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

        let mut request = self.client.put(self.url_for(path)).bearer_auth(token);

        if let Some(data) = data {
            request = request
                .header("Content-Type", "application/xml")
                .body(data.to_string());
        }

        let response = request.send().await?;
        let text = response.text().await?;
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

        let response = self
            .client
            .delete(self.url_for(path))
            .bearer_auth(token)
            .send()
            .await?;

        let text = response.text().await?;
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
        let _response = self.get(&path).await?;
        
        // Parse zones from response
        // Simplified - full implementation would parse XML properly
        Ok(Vec::new())
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
        let _response = self.get(&path).await?;
        
        // Parse records from response
        // Simplified - full implementation would parse XML and return actual records
        Ok(Vec::new())
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
