use std::future::Future;

use wasm_bindgen_futures::wasm_bindgen;

use web_sys::wasm_bindgen::JsValue;

pub fn execute<F: Future<Output = ()> + 'static>(f: F) {
    wasm_bindgen_futures::spawn_local(f);
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn send_post_request(url: &str, body: &str) -> Result<String, JsValue> {
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::wasm_bindgen::JsValue;
    use web_sys::{Request, RequestInit, Response};

    let body = body.to_string();

    log::debug!("Sending POST request to: {}", url);
    log::trace!("Body: {}", body);

    let opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_mode(web_sys::RequestMode::Cors);

    opts.set_body(&JsValue::from_str(&body));

    let request = Request::new_with_str_and_init(url, &opts)?;

    request.headers().set("Accept", "application/json")?;
    request.headers().set("Content-Type", "application/json")?;

    let window = web_sys::window().unwrap();

    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;

    assert!(resp_value.is_instance_of::<web_sys::Response>());

    let resp: Response = resp_value.dyn_into().unwrap();

    let content: String = JsFuture::from(resp.text()?).await?.as_string().unwrap();

    if !resp.ok() {
        return Err(JsValue::from_str(&format!(
            "Request failed with status: {} and error: {}",
            resp.status(),
            content
        )));
    }

    log::debug!("Response Content: {}", content);

    Ok(content)
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn send_get_request(url: &str) -> Result<String, JsValue> {
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::Request;
    use web_sys::RequestInit;
    use web_sys::RequestMode;
    use web_sys::Response;

    log::debug!("Sending GET request to: {}", url);

    let opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(RequestMode::Cors);

    let request = Request::new_with_str_and_init(url, &opts)?;

    request.headers().set("Accept", "application/json")?;

    let window = web_sys::window().unwrap();

    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;

    assert!(resp_value.is_instance_of::<web_sys::Response>());

    let resp: Response = resp_value.dyn_into().unwrap();

    if !resp.ok() {
        return Err(JsValue::from_str(&format!(
            "Request failed with status: {}",
            resp.status()
        )));
    }

    let content: String = JsFuture::from(resp.text()?).await?.as_string().unwrap();

    log::debug!("Response Content: {}", content);

    Ok(content)
}
