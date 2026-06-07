use loco_rs::prelude::*;
use serde_json::Value;

pub(crate) async fn fetch_json(
    client: &reqwest::Client,
    url: &str,
    user_agent: Option<&str>,
) -> Result<Value> {
    let mut request = client.get(url);
    if let Some(user_agent) = user_agent {
        request = request.header(reqwest::header::USER_AGENT, user_agent);
    }

    let response = request
        .send()
        .await
        .map_err(|err| Error::string(&format!("request failed for {url}: {err}")))?;
    let status = response.status();
    if !status.is_success() {
        return Err(Error::string(&format!(
            "request failed for {url}: {status}"
        )));
    }
    response
        .json::<Value>()
        .await
        .map_err(|err| Error::string(&format!("invalid JSON response from {url}: {err}")))
}
