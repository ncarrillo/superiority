//! Product-neutral JSON transport for the browser host.

use gloo_net::http::Request;
use serde::de::DeserializeOwned;

pub(crate) async fn get_json<T: DeserializeOwned>(url: &str) -> Result<T, String> {
    let response = Request::get(url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|error| format!("request failed: {error}"))?;
    if !response.ok() {
        return Err(format!("{} for {url}", response.status()));
    }
    response
        .json::<T>()
        .await
        .map_err(|error| format!("invalid response from {url}: {error}"))
}
