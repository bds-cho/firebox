use thiserror::Error;

use crate::dto::ErrorResponse;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("{0}")]
    Api(String),
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),
}

pub struct Client {
    base: String,
    inner: reqwest::Client,
}

impl Client {
    pub fn new(host: &str) -> Self {
        Self {
            base: format!("{}/api/v1", host.trim_end_matches('/')),
            inner: reqwest::Client::new(),
        }
    }

    pub async fn post_json<B: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<R, CliError> {
        let resp = self.inner.post(self.url(path)).json(body).send().await?;
        self.parse(resp).await
    }

    pub async fn post<R: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<R, CliError> {
        let resp = self.inner.post(self.url(path)).send().await?;
        self.parse(resp).await
    }

    pub async fn get<R: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<R, CliError> {
        let resp = self.inner.get(self.url(path)).send().await?;
        self.parse(resp).await
    }

    pub async fn delete(&self, path: &str) -> Result<(), CliError> {
        let resp = self.inner.delete(self.url(path)).send().await?;
        if resp.status().is_success() {
            return Ok(());
        }
        let err: ErrorResponse = resp.json().await?;
        Err(CliError::Api(err.error))
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    async fn parse<R: serde::de::DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> Result<R, CliError> {
        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            let err: ErrorResponse = resp.json().await?;
            Err(CliError::Api(err.error))
        }
    }
}
