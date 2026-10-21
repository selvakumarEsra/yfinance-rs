use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// A single SEC filing reported by Yahoo's `secFilings` quoteSummary module.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecFiling {
    /// Filing date.
    pub date: NaiveDate,
    /// Filing form type, such as `10-K`, `10-Q` or `8-K`.
    pub filing_type: String,
    /// Human-readable filing title, when Yahoo provides one.
    pub title: Option<String>,
    /// Link to the filing (Yahoo's EDGAR filing page), when provided.
    pub edgar_url: Option<String>,
    /// Every exhibit attached to the filing, in Yahoo's order.
    pub exhibits: Vec<SecFilingExhibit>,
}

/// An exhibit (document) attached to an SEC filing.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecFilingExhibit {
    /// Exhibit type, such as `10-K` or `EX-31.1`.
    pub exhibit_type: String,
    /// URL of the exhibit document.
    pub url: String,
    /// Direct download URL, when Yahoo provides one.
    pub download_url: Option<String>,
}
