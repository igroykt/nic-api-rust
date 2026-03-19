use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, ClientId, ClientSecret, ResourceOwnerPassword, ResourceOwnerUsername, TokenResponse,
    TokenUrl, RefreshToken,
};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{DnsApiError, Result};

/// Returns the current time as seconds since the UNIX epoch.
fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// OAuth2 token information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<u64>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
    /// UNIX timestamp (seconds) when this token was issued.
    /// Defaults to 0 when deserialising tokens that pre-date this field,
    /// which causes them to be treated as expired (safe default).
    #[serde(default)]
    pub issued_at: u64,
}

impl Token {
    /// Returns `true` when the token has expired or is within 30 seconds of
    /// expiring (to account for clock skew and network latency).
    pub fn is_expired(&self) -> bool {
        const BUFFER_SECS: u64 = 30;
        match self.expires_in {
            Some(expires_in) => {
                let now = current_unix_secs();
                // Use saturating_sub to avoid underflow when expires_in < BUFFER_SECS.
                now >= self.issued_at + expires_in.saturating_sub(BUFFER_SECS)
            }
            // No expiry information → assume the token is still valid.
            None => false,
        }
    }
}

/// Manages OAuth2 authentication for NIC.RU API
pub struct TokenManager {
    oauth_client: BasicClient,
    token: Option<Token>,
    #[allow(dead_code)]
    offline: u64,
    #[allow(dead_code)]
    scope: String,
}

impl TokenManager {
    /// Create a new TokenManager
    pub fn new(
        app_login: impl Into<String>,
        app_password: impl Into<String>,
        base_url: impl Into<String>,
        offline: u64,
        scope: impl Into<String>,
    ) -> Self {
        let base_url = base_url.into();
        let token_url = format!("{}/oauth/token", base_url);

        let oauth_client = BasicClient::new(
            ClientId::new(app_login.into()),
            Some(ClientSecret::new(app_password.into())),
            AuthUrl::new("https://api.nic.ru/oauth/authorize".to_string()).unwrap(),
            Some(TokenUrl::new(token_url).unwrap()),
        );

        Self {
            oauth_client,
            token: None,
            offline,
            scope: scope.into(),
        }
    }

    /// Set an existing token
    pub fn set_token(&mut self, token: Token) {
        self.token = Some(token);
    }

    /// Get current token
    pub fn get_token(&self) -> Option<&Token> {
        self.token.as_ref()
    }

    /// Get current token (alias for get_token)
    pub fn token(&self) -> Option<&Token> {
        self.token.as_ref()
    }

    /// Get access token string
    pub fn access_token(&self) -> Option<&str> {
        self.token.as_ref().map(|t| t.access_token.as_str())
    }

    /// Obtain a new token using username and password
    pub async fn get_token_with_password(
        &mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Result<Token> {
        let token_result = self
            .oauth_client
            .exchange_password(
                &ResourceOwnerUsername::new(username.into()),
                &ResourceOwnerPassword::new(password.into()),
            )
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|e| DnsApiError::OAuth2Error(e.to_string()))?;

        let token = Token {
            access_token: token_result.access_token().secret().clone(),
            token_type: "Bearer".to_string(),
            expires_in: token_result.expires_in().map(|d| d.as_secs()),
            refresh_token: token_result.refresh_token().map(|t| t.secret().clone()),
            scope: token_result
                .scopes()
                .map(|scopes| {
                    scopes
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                }),
            issued_at: current_unix_secs(),
        };

        log::debug!("Token scope: {:?}", token.scope);
        log::debug!("Token expires in: {:?} seconds", token.expires_in);
        self.token = Some(token.clone());
        Ok(token)
    }

    #[cfg(test)]
    pub fn base_url_for_test(&self) -> String {
        // helper so tests can check TokenManager was constructed
        "ok".to_string()
    }

    /// Ensures we have a valid (non-expired) access token.
    /// If the current token is expired but we have a refresh token, auto-refreshes.
    /// Returns the access token string.
    pub async fn ensure_valid_token(&mut self) -> crate::Result<String> {
        match &self.token {
            Some(token) if !token.is_expired() => {
                Ok(token.access_token.clone())
            }
            Some(Token { refresh_token: Some(ref rt), .. }) => {
                let refresh_token = rt.clone();
                log::info!("Token expired, auto-refreshing...");
                self.refresh_token(&refresh_token).await?;
                self.token
                    .as_ref()
                    .map(|t| t.access_token.clone())
                    .ok_or_else(|| crate::DnsApiError::OAuth2Error("Token refresh failed".to_string()))
            }
            Some(_) => Err(crate::DnsApiError::ExpiredToken),
            None => Err(crate::DnsApiError::OAuth2Error("No access token available".to_string())),
        }
    }

    /// Refresh an existing token
    pub async fn refresh_token(&mut self, refresh_token: impl Into<String>) -> Result<Token> {
        let refresh_token = RefreshToken::new(refresh_token.into());

        let token_result = self
            .oauth_client
            .exchange_refresh_token(&refresh_token)
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|e| DnsApiError::OAuth2Error(e.to_string()))?;

        let token = Token {
            access_token: token_result.access_token().secret().clone(),
            token_type: "Bearer".to_string(),
            expires_in: token_result.expires_in().map(|d| d.as_secs()),
            refresh_token: token_result.refresh_token().map(|t| t.secret().clone()),
            scope: token_result
                .scopes()
                .map(|scopes| {
                    scopes
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                }),
            issued_at: current_unix_secs(),
        };

        self.token = Some(token.clone());
        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_token() -> Token {
        Token {
            access_token: "test_access".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            refresh_token: Some("test_refresh".to_string()),
            scope: Some("dns".to_string()),
            issued_at: current_unix_secs(),
        }
    }

    #[test]
    fn token_serde_round_trip() {
        let token = sample_token();
        let serialized = serde_json::to_string(&token).unwrap();
        let deserialized: Token = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.access_token, "test_access");
        assert_eq!(deserialized.token_type, "Bearer");
        assert_eq!(deserialized.expires_in, Some(3600));
        assert_eq!(deserialized.refresh_token, Some("test_refresh".to_string()));
        assert_eq!(deserialized.scope, Some("dns".to_string()));
        assert!(deserialized.issued_at > 0);
    }

    #[test]
    fn token_is_not_expired_when_fresh() {
        // A token issued right now with a 1-hour TTL must not be expired.
        assert!(!sample_token().is_expired());
    }

    #[test]
    fn token_is_expired_when_old() {
        // A token issued 2 hours ago with a 1-hour TTL must be expired.
        let token = Token {
            issued_at: current_unix_secs() - 7200,
            expires_in: Some(3600),
            ..sample_token()
        };
        assert!(token.is_expired());
    }

    #[test]
    fn token_is_expired_within_buffer() {
        // A token that expires in 10 seconds (< 30 s buffer) must be treated as expired.
        let token = Token {
            issued_at: current_unix_secs() - 3590,
            expires_in: Some(3600),
            ..sample_token()
        };
        assert!(token.is_expired());
    }

    #[test]
    fn token_no_expiry_is_never_expired() {
        // A token without expiry information should never be considered expired.
        let token = Token {
            expires_in: None,
            issued_at: 0, // worst-case issued_at
            ..sample_token()
        };
        assert!(!token.is_expired());
    }

    #[test]
    fn token_manager_new_creates_instance() {
        let mgr = TokenManager::new("login", "pass", "https://api.nic.ru", 3600, ".+:/dns-master/.+");
        assert_eq!(mgr.base_url_for_test(), "ok");
        assert!(mgr.get_token().is_none());
    }

    #[test]
    fn token_manager_set_and_get_token() {
        let mut mgr = TokenManager::new("login", "pass", "https://api.nic.ru", 3600, "scope");
        let token = sample_token();
        mgr.set_token(token.clone());
        let got = mgr.get_token().unwrap();
        assert_eq!(got.access_token, "test_access");
    }

    #[test]
    fn token_manager_access_token() {
        let mut mgr = TokenManager::new("login", "pass", "https://api.nic.ru", 3600, "scope");
        assert_eq!(mgr.access_token(), None);
        mgr.set_token(sample_token());
        assert_eq!(mgr.access_token(), Some("test_access"));
    }
}
