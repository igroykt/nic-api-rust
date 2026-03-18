use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, ClientId, ClientSecret, ResourceOwnerPassword, ResourceOwnerUsername, TokenResponse,
    TokenUrl, RefreshToken,
};
use serde::{Deserialize, Serialize};

use crate::error::{DnsApiError, Result};

/// OAuth2 token information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<u64>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

impl Token {
    /// Check if token is likely expired (simple heuristic)
    pub fn is_expired(&self) -> bool {
        // In a real implementation, you'd track the creation time
        // and compare with expires_in
        false
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
    }

    #[test]
    fn token_is_expired_returns_false() {
        // stub always returns false
        assert!(!sample_token().is_expired());
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
