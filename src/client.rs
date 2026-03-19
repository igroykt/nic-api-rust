use quick_xml::Reader;
use quick_xml::events::Event;
use tokio::sync::Mutex;

use crate::auth::{Token, TokenManager};
use crate::error::{DnsApiError, Result};
use crate::models::{
    AaaaRecord, ARecord, CnameRecord, DnameRecord, DnsRecord, HinfoRecord,
    MxRecord, NaptrRecord, NicService, NicZone, NicZoneRevision, NsRecord, PtrRecord, RpRecord,
    SoaRecord, SrvRecord, TxtRecord,
};

/// Main API client for interacting with NIC.RU DNS API
pub struct DnsApi {
    http_client: reqwest::Client,
    base_url: String,
    token_manager: Mutex<TokenManager>,
    pub service_id: Option<String>,
    pub zone: Option<String>,
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
    /// * `service_id` - Optional default service name (NIC_SERVICE_ID)
    /// * `zone` - Optional default zone name
    pub fn new(
        app_login: impl Into<String>,
        app_password: impl Into<String>,
        token: Option<Token>,
        offline: Option<u64>,
        scope: Option<String>,
        service_id: Option<String>,
        zone: Option<String>,
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
            http_client: reqwest::Client::new(),
            base_url,
            token_manager: Mutex::new(token_manager),
            service_id,
            zone,
        }
    }

    /// Sets the default service name (NIC_SERVICE_ID) for DNS operations.
    pub fn set_service_id(&mut self, service: impl Into<String>) {
        self.service_id = Some(service.into());
    }

    /// Sets the default zone name for DNS operations.
    pub fn set_zone(&mut self, zone: impl Into<String>) {
        self.zone = Some(zone.into());
    }

    /// Get OAuth token using username and password
    pub async fn get_token(
        &self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Result<Token> {
        let mut tm = self.token_manager.lock().await;
        tm.get_token_with_password(username, password).await
    }

    /// Refresh OAuth token
    pub async fn refresh_token(&self, refresh_token: impl Into<String>) -> Result<Token> {
        let mut tm = self.token_manager.lock().await;
        tm.refresh_token(refresh_token).await
    }

    /// Get the current token (if any)
    pub async fn token(&self) -> Option<Token> {
        let tm = self.token_manager.lock().await;
        tm.token().cloned()
    }

    /// Set the current token directly
    pub async fn set_token(&self, token: Token) {
        let mut tm = self.token_manager.lock().await;
        tm.set_token(token);
    }

    /// Build full API URL
    fn url_for(&self, path: &str) -> String {
        format!("{}/dns-master/{}", self.base_url, path)
    }

    /// Get a valid token, auto-refreshing if expired
    async fn get_valid_token(&self) -> Result<String> {
        let mut tm = self.token_manager.lock().await;
        tm.ensure_valid_token().await
    }

    /// Refresh the token (used after 4097 error)
    async fn refresh_token_internal(&self) -> Result<()> {
        let mut tm = self.token_manager.lock().await;
        let refresh_tok = tm.token()
            .and_then(|t| t.refresh_token.clone())
            .ok_or_else(|| DnsApiError::OAuth2Error("No refresh token available for retry".to_string()))?;
        tm.refresh_token(&refresh_tok).await?;
        Ok(())
    }

    /// Execute a single HTTP request (no retry)
    async fn execute_request(
        &self,
        token: &str,
        method: reqwest::Method,
        url: &str,
        body: Option<&str>,
        content_type: &str,
    ) -> Result<String> {
        log::debug!("{} {}", method, url);
        let mut request = self.http_client.request(method, url)
            .header("Authorization", format!("Bearer {}", token));
        if let Some(body_str) = body {
            log::debug!("Request body: {}", body_str);
            request = request
                .header("Content-Type", content_type)
                .body(body_str.to_string());
        }
        let response = request.send().await?;
        log::debug!("Response status: {}", response.status());
        let text = response.text().await?;
        log::debug!("Response body: {}", text);
        self.check_response(&text)?;
        Ok(text)
    }

    /// Execute request with auto-retry on token expiry
    async fn execute_with_retry(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&str>,
        content_type: &str,
    ) -> Result<String> {
        let url = self.url_for(path);
        let token = self.get_valid_token().await?;
        let result = self.execute_request(&token, method.clone(), &url, body, content_type).await;
        match result {
            Err(DnsApiError::ExpiredToken) => {
                log::info!("Token expired during request, refreshing and retrying...");
                self.refresh_token_internal().await?;
                let new_token = self.get_valid_token().await?;
                self.execute_request(&new_token, method, &url, body, content_type).await
            }
            other => other,
        }
    }

    /// Make authenticated GET request
    async fn get(&self, path: &str) -> Result<String> {
        self.execute_with_retry(reqwest::Method::GET, path, None, "application/xml").await
    }

    /// Make authenticated POST request
    async fn post(&self, path: &str, body: Option<&str>) -> Result<String> {
        self.execute_with_retry(reqwest::Method::POST, path, body, "application/xml").await
    }

    /// Make authenticated PUT request
    async fn put(&self, path: &str, body: Option<&str>) -> Result<String> {
        self.execute_with_retry(reqwest::Method::PUT, path, body, "application/xml").await
    }

    /// Make authenticated DELETE request
    async fn delete(&self, path: &str) -> Result<String> {
        self.execute_with_retry(reqwest::Method::DELETE, path, None, "application/xml").await
    }

    /// Check API response for errors.
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
                            status = t.unescape().unwrap_or_default().to_string();
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
                            error_text = t.unescape().unwrap_or_default().to_string();
                        }
                    }
                    _ => {}
                },
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(DnsApiError::XmlError(e.to_string()));
                }
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

    /// Parse NicZone list from XML response
    fn parse_zones(&self, xml: &str) -> Result<Vec<NicZone>> {
        let mut zones = Vec::new();
        let mut reader = Reader::from_str(xml);
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

    /// Parse NicZoneRevision list from XML response
    fn parse_revisions(&self, xml: &str) -> Result<Vec<NicZoneRevision>> {
        let mut revisions = Vec::new();
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Empty(e)) | Ok(Event::Start(e)) if e.name().as_ref() == b"revision" => {
                    let mut date = String::new();
                    let mut ip = String::new();
                    let mut number: u32 = 0;

                    for attr in e.attributes().flatten() {
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("").to_string();
                        let value = attr.unescape_value().unwrap_or_default().to_string();
                        match key.as_str() {
                            "date" => date = value,
                            "ip" => ip = value,
                            "number" => number = value.parse().unwrap_or(0),
                            _ => {}
                        }
                    }
                    revisions.push(NicZoneRevision { date, ip, number });
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(DnsApiError::XmlError(e.to_string())),
                _ => {}
            }
            buf.clear();
        }

        Ok(revisions)
    }

    /// Parse a list of IP addresses from XML response (<ip> elements)
    fn parse_ip_list(&self, xml: &str) -> Result<Vec<String>> {
        let mut ips = Vec::new();
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let mut buf = Vec::new();
        let mut in_ip = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) if e.name().as_ref() == b"ip" => {
                    in_ip = true;
                }
                Ok(Event::Text(e)) if in_ip => {
                    ips.push(e.unescape().unwrap_or_default().to_string());
                }
                Ok(Event::End(e)) if e.name().as_ref() == b"ip" => {
                    in_ip = false;
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(DnsApiError::XmlError(e.to_string())),
                _ => {}
            }
            buf.clear();
        }

        Ok(ips)
    }

    /// Parse TTL value from SOA XML response
    fn parse_soa_ttl(&self, xml: &str) -> Result<u32> {
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let mut buf = Vec::new();
        let mut in_soa = false;
        let mut in_ttl = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    match e.name().as_ref() {
                        b"soa" => { in_soa = true; }
                        b"ttl" if in_soa => { in_ttl = true; }
                        _ => {}
                    }
                }
                Ok(Event::Text(e)) if in_ttl => {
                    let text = e.unescape().unwrap_or_default();
                    let ttl: u32 = text.parse().map_err(|_| {
                        DnsApiError::ApiError(format!("Invalid TTL value: {}", text))
                    })?;
                    return Ok(ttl);
                }
                Ok(Event::End(e)) => {
                    match e.name().as_ref() {
                        b"ttl" => { in_ttl = false; }
                        b"soa" => { in_soa = false; }
                        _ => {}
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(DnsApiError::XmlError(e.to_string())),
                _ => {}
            }
            buf.clear();
        }

        Err(DnsApiError::ApiError("No TTL found in SOA response".to_string()))
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
            .or(self.service_id.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;

        let path = format!("services/{}/zones", service);
        let response = self.get(&path).await?;
        self.parse_zones(&response)
    }

    /// List all zones across all services.
    /// GET /dns-master/zones
    pub async fn zones_all(&self) -> Result<Vec<NicZone>> {
        let response = self.get("zones").await?;
        self.parse_zones(&response)
    }

    /// Create a new DNS zone.
    /// PUT /dns-master/services/{service}/zones/{zone_name}
    pub async fn create_zone(
        &self,
        zone_name: &str,
        service: Option<&str>,
    ) -> Result<NicZone> {
        let service = service.or(self.service_id.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        let path = format!("services/{}/zones/{}", service, zone_name);
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\n<request><zone name=\"{}\"/></request>",
            zone_name
        );
        let response = self.put(&path, Some(&xml)).await?;
        let zones = self.parse_zones(&response)?;
        zones.into_iter().next()
            .ok_or_else(|| DnsApiError::ApiError("No zone returned after creation".to_string()))
    }

    /// Delete a DNS zone.
    /// DELETE /dns-master/services/{service}/zones/{zone_name}
    pub async fn delete_zone(
        &self,
        zone_name: &str,
        service: Option<&str>,
    ) -> Result<()> {
        let service = service.or(self.service_id.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        let path = format!("services/{}/zones/{}", service, zone_name);
        self.delete(&path).await?;
        Ok(())
    }

    /// Move a zone to another service.
    /// POST /dns-master/services/{service}/zones/{zone}/move?to={target_service}
    pub async fn move_zone(
        &self,
        zone: Option<&str>,
        target_service: &str,
        service: Option<&str>,
    ) -> Result<()> {
        let service = service.or(self.service_id.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        let zone = zone.or(self.zone.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No zone specified".to_string()))?;
        let path = format!("services/{}/zones/{}/move?to={}", service, zone, target_service);
        self.post(&path, None).await?;
        Ok(())
    }

    /// Export zone file in BIND format.
    /// GET /dns-master/services/{service}/zones/{zone}/export
    pub async fn zone_export(
        &self,
        service: Option<&str>,
        zone: Option<&str>,
    ) -> Result<String> {
        let service = service.or(self.service_id.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        let zone = zone.or(self.zone.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No zone specified".to_string()))?;
        let path = format!("services/{}/zones/{}/export", service, zone);
        let url = self.url_for(&path);

        // First attempt
        let token = self.get_valid_token().await?;
        let response = self.http_client.get(&url)
            .bearer_auth(&token)
            .send()
            .await?;

        // If we get 401/403, try refreshing token and retry
        if response.status() == reqwest::StatusCode::UNAUTHORIZED || response.status() == reqwest::StatusCode::FORBIDDEN {
            log::info!("Token expired during zone export, refreshing and retrying...");
            self.refresh_token_internal().await?;
            let new_token = self.get_valid_token().await?;
            let response = self.http_client.get(&url)
                .bearer_auth(&new_token)
                .send()
                .await?;
            Ok(response.text().await?)
        } else {
            Ok(response.text().await?)
        }
    }

    /// Import zone file in BIND format.
    /// POST /dns-master/services/{service}/zones/{zone}/import
    pub async fn zone_import(
        &self,
        content: &str,
        service: Option<&str>,
        zone: Option<&str>,
    ) -> Result<()> {
        let service = service.or(self.service_id.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        let zone = zone.or(self.zone.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No zone specified".to_string()))?;
        let path = format!("services/{}/zones/{}/import", service, zone);
        self.execute_with_retry(reqwest::Method::POST, &path, Some(content), "text/plain").await?;
        Ok(())
    }

    /// Rollback uncommitted changes for a zone.
    /// POST /dns-master/services/{service}/zones/{zone}/rollback
    pub async fn rollback(
        &self,
        service: Option<&str>,
        zone: Option<&str>,
    ) -> Result<()> {
        let service = service.or(self.service_id.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        let zone = zone.or(self.zone.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No zone specified".to_string()))?;
        let path = format!("services/{}/zones/{}/rollback", service, zone);
        self.post(&path, None).await?;
        Ok(())
    }

    /// Get the default TTL for a zone (from SOA record).
    /// GET /dns-master/services/{service}/zones/{zone}/soa
    pub async fn get_default_ttl(
        &self,
        service: Option<&str>,
        zone: Option<&str>,
    ) -> Result<u32> {
        let service = service.or(self.service_id.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        let zone = zone.or(self.zone.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No zone specified".to_string()))?;
        let path = format!("services/{}/zones/{}/soa", service, zone);
        let response = self.get(&path).await?;
        self.parse_soa_ttl(&response)
    }

    /// Set the default TTL for a zone.
    /// PUT /dns-master/services/{service}/zones/{zone}/soa
    pub async fn set_default_ttl(
        &self,
        ttl: u32,
        service: Option<&str>,
        zone: Option<&str>,
    ) -> Result<()> {
        let service = service.or(self.service_id.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        let zone = zone.or(self.zone.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No zone specified".to_string()))?;
        let path = format!("services/{}/zones/{}/soa", service, zone);
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\n<request><soa-list><soa><ttl>{}</ttl></soa></soa-list></request>",
            ttl
        );
        self.put(&path, Some(&xml)).await?;
        Ok(())
    }

    /// Get zone revision history.
    /// GET /dns-master/services/{service}/zones/{zone}/revisions
    pub async fn zone_revisions(
        &self,
        service: Option<&str>,
        zone: Option<&str>,
    ) -> Result<Vec<NicZoneRevision>> {
        let service = service.or(self.service_id.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        let zone = zone.or(self.zone.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No zone specified".to_string()))?;
        let path = format!("services/{}/zones/{}/revisions", service, zone);
        let response = self.get(&path).await?;
        self.parse_revisions(&response)
    }

    /// Get list of IP addresses allowed for zone transfer (AXFR).
    /// GET /dns-master/services/{service}/zones/{zone}/axfr
    pub async fn get_axfr_ips(
        &self,
        service: Option<&str>,
        zone: Option<&str>,
    ) -> Result<Vec<String>> {
        let service = service.or(self.service_id.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        let zone = zone.or(self.zone.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No zone specified".to_string()))?;
        let path = format!("services/{}/zones/{}/axfr", service, zone);
        let response = self.get(&path).await?;
        self.parse_ip_list(&response)
    }

    /// Set list of IP addresses allowed for zone transfer (AXFR).
    /// PUT /dns-master/services/{service}/zones/{zone}/axfr
    pub async fn set_axfr_ips(
        &self,
        ips: &[&str],
        service: Option<&str>,
        zone: Option<&str>,
    ) -> Result<()> {
        let service = service.or(self.service_id.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        let zone = zone.or(self.zone.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No zone specified".to_string()))?;
        let path = format!("services/{}/zones/{}/axfr", service, zone);
        let ip_xml: String = ips.iter().map(|ip| format!("<ip>{}</ip>", ip)).collect();
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\n<request><ip-list>{}</ip-list></request>",
            ip_xml
        );
        self.put(&path, Some(&xml)).await?;
        Ok(())
    }

    /// Get list of master DNS servers (for secondary zones).
    /// GET /dns-master/services/{service}/zones/{zone}/masters
    pub async fn get_masters(
        &self,
        service: Option<&str>,
        zone: Option<&str>,
    ) -> Result<Vec<String>> {
        let service = service.or(self.service_id.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        let zone = zone.or(self.zone.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No zone specified".to_string()))?;
        let path = format!("services/{}/zones/{}/masters", service, zone);
        let response = self.get(&path).await?;
        self.parse_ip_list(&response)
    }

    /// Set list of master DNS servers (for secondary zones).
    /// PUT /dns-master/services/{service}/zones/{zone}/masters
    pub async fn set_masters(
        &self,
        ips: &[&str],
        service: Option<&str>,
        zone: Option<&str>,
    ) -> Result<()> {
        let service = service.or(self.service_id.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        let zone = zone.or(self.zone.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No zone specified".to_string()))?;
        let path = format!("services/{}/zones/{}/masters", service, zone);
        let ip_xml: String = ips.iter().map(|ip| format!("<ip>{}</ip>", ip)).collect();
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\n<request><ip-list>{}</ip-list></request>",
            ip_xml
        );
        self.put(&path, Some(&xml)).await?;
        Ok(())
    }

    /// Commit changes to a DNS zone
    pub async fn commit(&self, service: Option<&str>, zone: Option<&str>) -> Result<()> {
        let service = service
            .or(self.service_id.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        
        let zone = zone
            .or(self.zone.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No zone specified".to_string()))?;

        let path = format!("services/{}/zones/{}/commit", service, zone);
        self.post(&path, None).await?;
        Ok(())
    }

    /// Parse DNS records from an XML response string containing `<rr>` elements.
    fn parse_dns_records(&self, xml: &str) -> Result<Vec<DnsRecord>> {
        let mut records = Vec::new();
        let mut reader = Reader::from_str(xml);
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
        // SOA
        let mut in_soa = false;
        let mut rr_soa_mname = String::new();
        let mut rr_soa_rname = String::new();
        let mut rr_soa_serial: u32 = 0;
        let mut rr_soa_refresh: u32 = 0;
        let mut rr_soa_retry: u32 = 0;
        let mut rr_soa_expire: u32 = 0;
        let mut rr_soa_minimum: u32 = 0;
        let mut in_soa_mname = false;
        let mut in_soa_rname = false;
        let mut in_soa_serial = false;
        let mut in_soa_refresh = false;
        let mut in_soa_retry = false;
        let mut in_soa_expire = false;
        let mut in_soa_minimum = false;
        // HINFO
        let mut in_hinfo = false;
        let mut rr_hinfo_hardware = String::new();
        let mut rr_hinfo_os = String::new();
        let mut in_hinfo_hardware = false;
        let mut in_hinfo_os = false;
        // NAPTR
        let mut in_naptr = false;
        let mut rr_naptr_order: u16 = 0;
        let mut rr_naptr_preference: u16 = 0;
        let mut rr_naptr_flags = String::new();
        let mut rr_naptr_service = String::new();
        let mut rr_naptr_regexp: Option<String> = None;
        let mut rr_naptr_replacement: Option<String> = None;
        let mut in_naptr_order = false;
        let mut in_naptr_preference = false;
        let mut in_naptr_flags = false;
        let mut in_naptr_service = false;
        let mut in_naptr_regexp = false;
        let mut in_naptr_replacement = false;
        // RP
        let mut in_rp = false;
        let mut rr_rp_mbox = String::new();
        let mut rr_rp_txt = String::new();
        let mut in_rp_mbox = false;
        let mut in_rp_txt = false;
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
                            in_soa = false;
                            rr_soa_mname.clear(); rr_soa_rname.clear();
                            rr_soa_serial = 0; rr_soa_refresh = 0; rr_soa_retry = 0;
                            rr_soa_expire = 0; rr_soa_minimum = 0;
                            in_hinfo = false;
                            rr_hinfo_hardware.clear(); rr_hinfo_os.clear();
                            in_naptr = false;
                            rr_naptr_order = 0; rr_naptr_preference = 0;
                            rr_naptr_flags.clear(); rr_naptr_service.clear();
                            rr_naptr_regexp = None; rr_naptr_replacement = None;
                            in_rp = false;
                            rr_rp_mbox.clear(); rr_rp_txt.clear();
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
                        "soa" if in_rr => { in_soa = true; }
                        "mname" if in_soa => { in_soa_mname = true; }
                        "rname" if in_soa => { in_soa_rname = true; }
                        "serial" if in_soa => { in_soa_serial = true; }
                        "refresh" if in_soa => { in_soa_refresh = true; }
                        "retry" if in_soa => { in_soa_retry = true; }
                        "expire" if in_soa => { in_soa_expire = true; }
                        "minimum" if in_soa => { in_soa_minimum = true; }
                        "hinfo" if in_rr => { in_hinfo = true; }
                        "hardware" if in_hinfo => { in_hinfo_hardware = true; }
                        "os" if in_hinfo => { in_hinfo_os = true; }
                        "naptr" if in_rr => { in_naptr = true; }
                        "order" if in_naptr => { in_naptr_order = true; }
                        "preference" if in_naptr => { in_naptr_preference = true; }
                        "flags" if in_naptr => { in_naptr_flags = true; }
                        "service" if in_naptr => { in_naptr_service = true; }
                        "regexp" if in_naptr => { in_naptr_regexp = true; }
                        "replacement" if in_naptr => { in_naptr_replacement = true; }
                        "rp" if in_rr => { in_rp = true; }
                        "mbox-dname" if in_rp => { in_rp_mbox = true; }
                        "txt-dname" if in_rp => { in_rp_txt = true; }
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
                                    ttl: rr_ttl,
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
                                "SOA" => Some(DnsRecord::SOA(SoaRecord {
                                    id: rr_id,
                                    name: rr_name.clone(),
                                    idn_name: None,
                                    ttl: rr_ttl,
                                    serial: rr_soa_serial,
                                    refresh: rr_soa_refresh,
                                    retry: rr_soa_retry,
                                    expire: rr_soa_expire,
                                    minimum: rr_soa_minimum,
                                    mname: rr_soa_mname.clone(),
                                    rname: rr_soa_rname.clone(),
                                })),
                                "HINFO" => Some(DnsRecord::HINFO(HinfoRecord {
                                    id: rr_id,
                                    name: rr_name.clone(),
                                    idn_name: None,
                                    ttl: rr_ttl,
                                    hardware: rr_hinfo_hardware.clone(),
                                    os: rr_hinfo_os.clone(),
                                })),
                                "NAPTR" => Some(DnsRecord::NAPTR(NaptrRecord {
                                    id: rr_id,
                                    name: rr_name.clone(),
                                    idn_name: None,
                                    ttl: rr_ttl,
                                    order: rr_naptr_order,
                                    preference: rr_naptr_preference,
                                    flags: rr_naptr_flags.clone(),
                                    service: rr_naptr_service.clone(),
                                    regexp: rr_naptr_regexp.clone(),
                                    replacement: rr_naptr_replacement.clone(),
                                })),
                                "RP" => Some(DnsRecord::RP(RpRecord {
                                    id: rr_id,
                                    name: rr_name.clone(),
                                    idn_name: None,
                                    ttl: rr_ttl,
                                    mbox: rr_rp_mbox.clone(),
                                    txt: rr_rp_txt.clone(),
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
                        "preference" if in_mx => { in_mx_preference = false; }
                        "preference" if in_naptr => { in_naptr_preference = false; }
                        "exchange" => { in_mx_exchange = false; }
                        "priority" => { in_srv_priority = false; }
                        "weight" => { in_srv_weight = false; }
                        "port" => { in_srv_port = false; }
                        "target" => { in_srv_target = false; }
                        "soa" => { in_soa = false; }
                        "mname" => { in_soa_mname = false; }
                        "rname" => { in_soa_rname = false; }
                        "serial" => { in_soa_serial = false; }
                        "refresh" => { in_soa_refresh = false; }
                        "retry" => { in_soa_retry = false; }
                        "expire" => { in_soa_expire = false; }
                        "minimum" => { in_soa_minimum = false; }
                        "hinfo" => { in_hinfo = false; }
                        "hardware" => { in_hinfo_hardware = false; }
                        "os" => { in_hinfo_os = false; }
                        "naptr" => { in_naptr = false; }
                        "order" if in_naptr => { in_naptr_order = false; }
                        "flags" => { in_naptr_flags = false; }
                        "service" if in_naptr => { in_naptr_service = false; }
                        "regexp" => { in_naptr_regexp = false; }
                        "replacement" => { in_naptr_replacement = false; }
                        "rp" => { in_rp = false; }
                        "mbox-dname" => { in_rp_mbox = false; }
                        "txt-dname" => { in_rp_txt = false; }
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
                    } else if in_soa_mname {
                        rr_soa_mname = text;
                    } else if in_soa_rname {
                        rr_soa_rname = text;
                    } else if in_soa_serial {
                        rr_soa_serial = text.parse().unwrap_or(0);
                    } else if in_soa_refresh {
                        rr_soa_refresh = text.parse().unwrap_or(0);
                    } else if in_soa_retry {
                        rr_soa_retry = text.parse().unwrap_or(0);
                    } else if in_soa_expire {
                        rr_soa_expire = text.parse().unwrap_or(0);
                    } else if in_soa_minimum {
                        rr_soa_minimum = text.parse().unwrap_or(0);
                    } else if in_hinfo_hardware {
                        rr_hinfo_hardware = text;
                    } else if in_hinfo_os {
                        rr_hinfo_os = text;
                    } else if in_naptr_order {
                        rr_naptr_order = text.parse().unwrap_or(0);
                    } else if in_naptr_preference {
                        rr_naptr_preference = text.parse().unwrap_or(0);
                    } else if in_naptr_flags {
                        rr_naptr_flags = text;
                    } else if in_naptr_service {
                        rr_naptr_service = text;
                    } else if in_naptr_regexp {
                        rr_naptr_regexp = Some(text);
                    } else if in_naptr_replacement {
                        rr_naptr_replacement = Some(text);
                    } else if in_rp_mbox {
                        rr_rp_mbox = text;
                    } else if in_rp_txt {
                        rr_rp_txt = text;
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

    /// Get DNS records for a zone.
    /// GET /dns-master/services/{service}/zones/{zone}/records
    pub async fn records(&self, service: Option<&str>, zone: Option<&str>) -> Result<Vec<DnsRecord>> {
        let service = service
            .or(self.service_id.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        
        let zone = zone
            .or(self.zone.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No zone specified".to_string()))?;

        let path = format!("services/{}/zones/{}/records", service, zone);
        let response = self.get(&path).await?;

        self.parse_dns_records(&response)
    }

    /// Add DNS record(s) to a zone
    pub async fn add_record(
        &self,
        records: Vec<DnsRecord>,
        service: Option<&str>,
        zone: Option<&str>,
    ) -> Result<Vec<DnsRecord>> {
        let service = service
            .or(self.service_id.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        
        let zone = zone
            .or(self.zone.as_deref())
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
        let response = self.put(&path, Some(&xml)).await?;

        self.parse_dns_records(&response)
    }

    /// Delete a DNS record by ID
    pub async fn delete_record(
        &self,
        record_id: u64,
        service: Option<&str>,
        zone: Option<&str>,
    ) -> Result<()> {
        let service = service
            .or(self.service_id.as_deref())
            .ok_or_else(|| DnsApiError::ApiError("No service specified".to_string()))?;
        
        let zone = zone
            .or(self.zone.as_deref())
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
            issued_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        DnsApi::new("app_login", "app_password", Some(token), None, None, None, None)
    }

    #[test]
    fn dns_api_new_default_params() {
        let api = create_test_api();
        assert_eq!(api.base_url, "https://api.nic.ru");
        assert_eq!(api.service_id, None);
        assert_eq!(api.zone, None);
        // Check token is stored via try_lock (no async runtime needed)
        let tm = api.token_manager.try_lock().unwrap();
        assert_eq!(tm.access_token(), Some("test_token"));
    }

    #[test]
    fn dns_api_new_custom_params() {
        let api = DnsApi::new("login", "pass", None, Some(7200), Some("custom_scope".to_string()), None, None);
        assert_eq!(api.base_url, "https://api.nic.ru");
        let tm = api.token_manager.try_lock().unwrap();
        assert_eq!(tm.access_token(), None);
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
    fn check_response_non_xml_errors() {
        let api = create_test_api();
        // Non-XML content (e.g. BIND zone file) should now return XmlError in strict mode.
        // zone_export bypasses check_response entirely.
        let bind_data = "$ORIGIN example.com.\n@ 3600 IN SOA ns1. admin. 1 3600 900 604800 300";
        assert!(matches!(api.check_response(bind_data), Err(DnsApiError::XmlError(_))));
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

    #[test]
    fn parse_zones_from_xml() {
        let api = create_test_api();
        let xml = r#"<response><status>success</status><data><zone admin="admin" enable="true" has-changes="false" has-primary="true" id="1" idn-name="example.com" name="example.com" payer="payer" service="mysvc" /></data></response>"#;
        let zones = api.parse_zones(xml).unwrap();
        assert_eq!(zones.len(), 1);
        assert_eq!(zones[0].name, "example.com");
        assert_eq!(zones[0].service, "mysvc");
    }

    #[test]
    fn parse_revisions_from_xml() {
        let api = create_test_api();
        let xml = r#"<response><status>success</status><data><revision date="2023-01-01" ip="1.2.3.4" number="42" /></data></response>"#;
        let revisions = api.parse_revisions(xml).unwrap();
        assert_eq!(revisions.len(), 1);
        assert_eq!(revisions[0].date, "2023-01-01");
        assert_eq!(revisions[0].ip, "1.2.3.4");
        assert_eq!(revisions[0].number, 42);
    }

    #[test]
    fn parse_ip_list_from_xml() {
        let api = create_test_api();
        let xml = r#"<response><status>success</status><data><ip-list><ip>1.2.3.4</ip><ip>5.6.7.8</ip></ip-list></data></response>"#;
        let ips = api.parse_ip_list(xml).unwrap();
        assert_eq!(ips, vec!["1.2.3.4", "5.6.7.8"]);
    }

    #[test]
    fn parse_soa_ttl_from_xml() {
        let api = create_test_api();
        let xml = r#"<response><status>success</status><data><soa><ttl>3600</ttl></soa></data></response>"#;
        let ttl = api.parse_soa_ttl(xml).unwrap();
        assert_eq!(ttl, 3600);
    }
}
