use dotenvy_macro::dotenv;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, Request, RequestInit, RequestMode, Response};

const CLIENT_ID: &str = dotenv!("GOOGLE_CLIENT_ID");

const SCOPES: &str =
    "https://www.googleapis.com/auth/drive.file https://www.googleapis.com/auth/drive.readonly";

#[wasm_bindgen(inline_js = "
export function requestGoogleAccessToken(clientId, scope) {
    return new Promise((resolve, reject) => {
        if (typeof google === 'undefined' || !google.accounts) {
            reject('Google Identity Services not loaded');
            return;
        }
        const client = google.accounts.oauth2.initTokenClient({
            client_id: clientId,
            scope: scope,
            callback: (response) => {
                if (response.error) {
                    reject(response.error);
                } else {
                    resolve(response.access_token);
                }
            },
            error_callback: (err) => {
                reject(err.message || 'OAuth error');
            },
        });
        client.requestAccessToken();
    });
}
")]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn requestGoogleAccessToken(client_id: &str, scope: &str) -> Result<JsValue, JsValue>;
}

pub async fn get_access_token() -> Result<String, String> {
    let result = requestGoogleAccessToken(CLIENT_ID, SCOPES)
        .await
        .map_err(|e| format!("{:?}", e))?;
    result
        .as_string()
        .ok_or_else(|| "Token was not a string".to_owned())
}

pub struct SaveResult {
    pub file_id: String,
    pub name: String,
}

pub async fn save_file(
    token: &str,
    content: &str,
    filename: &str,
    file_id: Option<&str>,
) -> Result<SaveResult, String> {
    let boundary = "kwocs_boundary";

    let metadata = if file_id.is_some() {
        // Update: no name in metadata (keeps existing name unless we want to rename)
        "{}".to_owned()
    } else {
        format!(r#"{{"name":"{}","mimeType":"text/markdown"}}"#, filename)
    };

    let body = format!(
        "--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadata}\r\n--{boundary}\r\nContent-Type: text/markdown\r\n\r\n{content}\r\n--{boundary}--"
    );

    let url = match file_id {
        Some(id) => {
            format!("https://www.googleapis.com/upload/drive/v3/files/{id}?uploadType=multipart")
        }
        None => "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart".to_owned(),
    };

    let method = if file_id.is_some() { "PATCH" } else { "POST" };

    let headers = Headers::new().map_err(|e| format!("{:?}", e))?;
    headers
        .set(
            "Content-Type",
            &format!("multipart/related; boundary={boundary}"),
        )
        .map_err(|e| format!("{:?}", e))?;
    headers
        .set("Authorization", &format!("Bearer {token}"))
        .map_err(|e| format!("{:?}", e))?;

    let opts = RequestInit::new();
    opts.set_method(method);
    opts.set_headers(&headers);
    opts.set_mode(RequestMode::Cors);
    opts.set_body(&wasm_bindgen::JsValue::from_str(&body));

    let request = Request::new_with_str_and_init(&url, &opts).map_err(|e| format!("{:?}", e))?;

    let window = web_sys::window().ok_or("No window")?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("{:?}", e))?;
    let resp: Response = resp_value.dyn_into().map_err(|e| format!("{:?}", e))?;

    if resp.status() == 401 {
        return Err("Session expired. Please try again.".to_owned());
    }

    if !resp.ok() {
        let text = JsFuture::from(resp.text().map_err(|e| format!("{:?}", e))?)
            .await
            .map_err(|e| format!("{:?}", e))?;
        return Err(format!(
            "Drive API error {}: {}",
            resp.status(),
            text.as_string().unwrap_or_default()
        ));
    }

    let json = JsFuture::from(resp.json().map_err(|e| format!("{:?}", e))?)
        .await
        .map_err(|e| format!("{:?}", e))?;

    let id = js_sys::Reflect::get(&json, &"id".into())
        .map_err(|e| format!("{:?}", e))?
        .as_string()
        .unwrap_or_default();
    let name = js_sys::Reflect::get(&json, &"name".into())
        .map_err(|e| format!("{:?}", e))?
        .as_string()
        .unwrap_or_default();

    Ok(SaveResult { file_id: id, name })
}

pub async fn open_file(token: &str, file_id: &str) -> Result<String, String> {
    let url = format!("https://www.googleapis.com/drive/v3/files/{file_id}?alt=media");

    let headers = Headers::new().map_err(|e| format!("{:?}", e))?;
    headers
        .set("Authorization", &format!("Bearer {token}"))
        .map_err(|e| format!("{:?}", e))?;

    let opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_headers(&headers);
    opts.set_mode(RequestMode::Cors);

    let request = Request::new_with_str_and_init(&url, &opts).map_err(|e| format!("{:?}", e))?;

    let window = web_sys::window().ok_or("No window")?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("{:?}", e))?;
    let resp: Response = resp_value.dyn_into().map_err(|e| format!("{:?}", e))?;

    if resp.status() == 401 {
        return Err("Session expired. Please try again.".to_owned());
    }

    if !resp.ok() {
        let text = JsFuture::from(resp.text().map_err(|e| format!("{:?}", e))?)
            .await
            .map_err(|e| format!("{:?}", e))?;
        return Err(format!(
            "Drive API error {}: {}",
            resp.status(),
            text.as_string().unwrap_or_default()
        ));
    }

    let text = JsFuture::from(resp.text().map_err(|e| format!("{:?}", e))?)
        .await
        .map_err(|e| format!("{:?}", e))?;
    text.as_string()
        .ok_or_else(|| "Response was not a string".to_owned())
}

pub fn extract_file_id(url: &str) -> Option<String> {
    // Pattern: /d/{id}/
    if let Some(pos) = url.find("/d/") {
        let rest = &url[pos + 3..];
        let id = rest.split('/').next()?;
        if !id.is_empty() {
            return Some(id.to_owned());
        }
    }
    // Pattern: ?id={id} or &id={id}
    if let Some(pos) = url.find("id=") {
        let rest = &url[pos + 3..];
        let id = rest.split('&').next().unwrap_or(rest);
        if !id.is_empty() {
            return Some(id.to_owned());
        }
    }
    None
}
