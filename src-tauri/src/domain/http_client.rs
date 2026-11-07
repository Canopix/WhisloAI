use reqwest::{Client, RequestBuilder, Response};
use std::time::Duration;

const REQUEST_TIMEOUT_SECS: u64 = 30;
const CONNECT_TIMEOUT_SECS: u64 = 10;
const MAX_RETRIES: u32 = 2;
const INITIAL_BACKOFF_MS: u64 = 500;

#[derive(Debug, Clone, Copy)]
pub(crate) struct RetryConfig {
    pub(crate) max_retries: u32,
    pub(crate) initial_backoff_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: MAX_RETRIES,
            initial_backoff_ms: INITIAL_BACKOFF_MS,
        }
    }
}

pub(crate) fn create_http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Could not create HTTP client: {e}"))
}

fn is_retryable_error(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect()
}

fn is_retryable_status(status: u16) -> bool {
    status >= 500 && status < 600
}

pub(crate) async fn send_with_retry(
    builder: RequestBuilder,
    config: RetryConfig,
) -> Result<Response, String> {
    let mut last_error: Option<String> = None;
    
    for attempt in 0..=config.max_retries {
        let request_clone = match builder.try_clone() {
            Some(req) => req,
            None => {
                return Err(last_error.unwrap_or_else(|| 
                    "Could not clone request for retry".to_string()
                ));
            }
        };

        match request_clone.send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    return Ok(response);
                }

                if !is_retryable_status(status.as_u16()) {
                    return Err(format!(
                        "HTTP {}: Request failed with non-retryable status",
                        status.as_u16()
                    ));
                }

                last_error = Some(format!(
                    "HTTP {}: Server error",
                    status.as_u16()
                ));
            }
            Err(e) => {
                if !is_retryable_error(&e) {
                    return Err(format!("Request failed: {e}"));
                }

                last_error = Some(format!("Request failed: {e}"));
            }
        }

        if attempt < config.max_retries {
            let backoff_ms = config.initial_backoff_ms * 2_u64.pow(attempt);
            tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
        }
    }

    Err(last_error.unwrap_or_else(|| "Request failed after retries".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, MAX_RETRIES);
        assert_eq!(config.initial_backoff_ms, INITIAL_BACKOFF_MS);
    }

    #[test]
    fn test_is_retryable_status() {
        assert!(!is_retryable_status(200));
        assert!(!is_retryable_status(400));
        assert!(!is_retryable_status(404));
        assert!(is_retryable_status(500));
        assert!(is_retryable_status(502));
        assert!(is_retryable_status(503));
        assert!(!is_retryable_status(600));
    }

    #[tokio::test]
    async fn test_create_http_client() {
        let client = create_http_client();
        assert!(client.is_ok());
    }
}
