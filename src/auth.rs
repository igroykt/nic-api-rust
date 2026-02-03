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

        self.token = Some(token.clone());
        Ok(token)
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
