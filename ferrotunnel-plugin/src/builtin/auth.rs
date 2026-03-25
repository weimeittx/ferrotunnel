use crate::traits::{Plugin, PluginAction, RequestContext};
use async_trait::async_trait;
use std::collections::HashSet;

/// Token-based authentication plugin
pub struct TokenAuthPlugin {
    valid_tokens: HashSet<String>,
    header_name: String,
}

impl TokenAuthPlugin {
    pub fn new(tokens: Vec<String>) -> Self {
        Self {
            valid_tokens: tokens.into_iter().collect(),
            header_name: "X-Tunnel-Token".to_string(),
        }
    }

    #[must_use]
    pub fn with_header_name(mut self, name: String) -> Self {
        self.header_name = name;
        self
    }
}

#[async_trait]
impl Plugin for TokenAuthPlugin {
    fn name(&self) -> &str {
        "token-auth"
    }

    async fn on_request(
        &self,
        req: &mut http::Request<()>,
        _ctx: &RequestContext,
    ) -> Result<PluginAction, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let header_token = req
            .headers()
            .get(&self.header_name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);

        let token = header_token.or_else(|| {
            req.uri().query().and_then(|q| {
                form_urlencoded::parse(q.as_bytes())
                    .find(|(k, _)| k == "_token")
                    .map(|(_, v)| v.into_owned())
            })
        });

        match token.as_deref() {
            Some(t) if self.valid_tokens.contains(t) => Ok(PluginAction::Continue),
            _ => Ok(PluginAction::Reject {
                status: 401,
                reason: format!("Invalid or missing {}", self.header_name),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_valid_token_allows_request() {
        let plugin = TokenAuthPlugin::new(vec!["secret123".to_string()]);

        let mut req = http::Request::builder()
            .header("X-Tunnel-Token", "secret123")
            .uri("/")
            .body(())
            .unwrap();

        let ctx = RequestContext {
            tunnel_id: "test".into(),
            session_id: "session".into(),
            remote_addr: "127.0.0.1:1234".parse().unwrap(),
            timestamp: std::time::SystemTime::now(),
        };

        let action = plugin.on_request(&mut req, &ctx).await.unwrap();
        assert_eq!(action, PluginAction::Continue);
    }

    #[tokio::test]
    async fn test_invalid_token_rejects_request() {
        let plugin = TokenAuthPlugin::new(vec!["secret123".to_string()]);

        let mut req = http::Request::builder()
            .header("X-Tunnel-Token", "wrong")
            .uri("/")
            .body(())
            .unwrap();

        let ctx = RequestContext {
            tunnel_id: "test".into(),
            session_id: "session".into(),
            remote_addr: "127.0.0.1:1234".parse().unwrap(),
            timestamp: std::time::SystemTime::now(),
        };

        let action = plugin.on_request(&mut req, &ctx).await.unwrap();

        match action {
            PluginAction::Reject { status, .. } => assert_eq!(status, 401),
            _ => panic!("Expected Reject"),
        }
    }

    #[tokio::test]
    async fn test_token_from_query_param() {
        let plugin = TokenAuthPlugin::new(vec!["secret123".to_string()]);

        let mut req = http::Request::builder()
            .uri("/path?_token=secret123&other=value")
            .body(())
            .unwrap();

        let ctx = RequestContext {
            tunnel_id: "test".into(),
            session_id: "session".into(),
            remote_addr: "127.0.0.1:1234".parse().unwrap(),
            timestamp: std::time::SystemTime::now(),
        };

        let action = plugin.on_request(&mut req, &ctx).await.unwrap();
        assert_eq!(action, PluginAction::Continue);
    }

    #[tokio::test]
    async fn test_header_token_takes_precedence_over_query() {
        let plugin = TokenAuthPlugin::new(vec!["header-tok".to_string()]);

        let mut req = http::Request::builder()
            .header("X-Tunnel-Token", "header-tok")
            .uri("/path?_token=query-tok")
            .body(())
            .unwrap();

        let ctx = RequestContext {
            tunnel_id: "test".into(),
            session_id: "session".into(),
            remote_addr: "127.0.0.1:1234".parse().unwrap(),
            timestamp: std::time::SystemTime::now(),
        };

        let action = plugin.on_request(&mut req, &ctx).await.unwrap();
        assert_eq!(action, PluginAction::Continue);
    }

    #[tokio::test]
    async fn test_invalid_query_token_rejects() {
        let plugin = TokenAuthPlugin::new(vec!["secret123".to_string()]);

        let mut req = http::Request::builder()
            .uri("/path?_token=wrong")
            .body(())
            .unwrap();

        let ctx = RequestContext {
            tunnel_id: "test".into(),
            session_id: "session".into(),
            remote_addr: "127.0.0.1:1234".parse().unwrap(),
            timestamp: std::time::SystemTime::now(),
        };

        let action = plugin.on_request(&mut req, &ctx).await.unwrap();
        match action {
            PluginAction::Reject { status, .. } => assert_eq!(status, 401),
            _ => panic!("Expected Reject"),
        }
    }
}
