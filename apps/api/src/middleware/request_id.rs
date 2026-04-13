use axum::{
    body::Body,
    http::{HeaderMap, HeaderName, HeaderValue, Request},
    middleware::Next,
    response::Response,
};
use application::audit::{AuditContext, scope_audit_context};
use uuid::Uuid;

pub const X_REQUEST_ID: &str = "x-request-id";
pub const X_CORRELATION_ID: &str = "x-correlation-id";

#[derive(Debug, Clone)]
pub struct RequestContext {
    pub request_id: String,
    pub correlation_id: String,
}

pub async fn attach_request_context(mut request: Request<Body>, next: Next) -> Response {
    let request_id = header_or_generate(request.headers(), X_REQUEST_ID);
    let correlation_id = header_or_generate(request.headers(), X_CORRELATION_ID);

    set_header(request.headers_mut(), X_REQUEST_ID, &request_id);
    set_header(request.headers_mut(), X_CORRELATION_ID, &correlation_id);
    request.extensions_mut().insert(RequestContext {
        request_id: request_id.clone(),
        correlation_id: correlation_id.clone(),
    });

    let mut response = scope_audit_context(
        AuditContext { request_id: request_id.clone(), correlation_id: correlation_id.clone() },
        next.run(request),
    )
    .await;
    set_header(response.headers_mut(), X_REQUEST_ID, &request_id);
    set_header(response.headers_mut(), X_CORRELATION_ID, &correlation_id);
    response
}

fn header_or_generate(headers: &HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn set_header(headers: &mut HeaderMap, name: &str, value: &str) {
    if let (Ok(header_name), Ok(header_value)) =
        (HeaderName::from_bytes(name.as_bytes()), HeaderValue::from_str(value))
    {
        headers.insert(header_name, header_value);
    }
}
