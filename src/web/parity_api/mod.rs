use crate::web::api::WebResponse;
use crate::web::http::{HttpRequest, HttpResponse, StatusCode};

pub(super) fn query_str(req: &HttpRequest, key: &str, default_val: &str) -> String {
    req.query_params
        .get(key)
        .map(|s| urlencoding::decode(s).map(|d| d.into_owned()).unwrap_or_else(|_| s.clone()))
        .unwrap_or_else(|| default_val.to_string())
}

pub(super) fn check_auth(req: &HttpRequest, auth_token: &Option<String>) -> Option<HttpResponse> {
    if let Some(expected_token) = auth_token.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let auth_header = req.headers.get("authorization").cloned().unwrap_or_default();
        let token = auth_header.strip_prefix("Bearer ").unwrap_or(&auth_header);
        if !crate::mesh::auth::constant_time_eq(token, expected_token) {
            return Some(WebResponse::error(
                StatusCode::UNAUTHORIZED,
                "Unauthorized: invalid auth token",
            ));
        }
    }
    None
}

pub mod core_endpoints;
pub mod gov_body_endpoints;
pub mod ops_endpoints;

pub use core_endpoints::*;
pub use gov_body_endpoints::*;
pub use ops_endpoints::*;
