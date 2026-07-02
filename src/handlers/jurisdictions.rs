use crate::verifier::jurisdiction::{
    DEFAULT_JURISDICTION, JurisdictionInfo, MultiJurisdictionMatcher,
};
use axum::Json;
use serde::Serialize;

/// Response body for `GET /jurisdictions`.
#[derive(Debug, Serialize)]
pub struct JurisdictionsResponse {
    /// Registered jurisdictions, sorted by code.
    pub jurisdictions: Vec<JurisdictionInfo>,
    /// Code selected when a request omits `jurisdiction`.
    pub default: String,
}

/// GET /jurisdictions
///
/// Discovery endpoint listing the jurisdictions the multi-jurisdiction matcher
/// supports, plus the default code used when a request omits `jurisdiction`.
///
/// Example response:
/// ```json
/// {
///   "jurisdictions": [
///     {"code": "EU", "display_name": "欧州連合法 / European Union Law (GDPR)", "is_populated": true},
///     {"code": "JP", "display_name": "日本法 / Japanese Law", "is_populated": true},
///     {"code": "US", "display_name": "米国連邦法 / United States Federal Law", "is_populated": true}
///   ],
///   "default": "JP"
/// }
/// ```
pub async fn list_jurisdictions() -> Json<JurisdictionsResponse> {
    let registry = MultiJurisdictionMatcher::new();
    Json(JurisdictionsResponse {
        jurisdictions: registry.available(),
        default: DEFAULT_JURISDICTION.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_jurisdictions_shape() {
        let Json(resp) = list_jurisdictions().await;
        assert_eq!(resp.default, "JP");
        assert_eq!(resp.jurisdictions.len(), 3);
        let codes: Vec<&str> = resp.jurisdictions.iter().map(|j| j.code.as_str()).collect();
        assert_eq!(codes, vec!["EU", "JP", "US"]);

        // Serializes to the documented shape.
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"jurisdictions\""));
        assert!(json.contains("\"default\":\"JP\""));
        assert!(json.contains("\"is_populated\":true"));
    }
}
