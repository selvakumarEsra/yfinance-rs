use serde::Deserialize;

use crate::core::wire::{BufferedWireValue, WireValue};

#[derive(Deserialize)]
pub struct V10Result {
    #[serde(rename = "secFilings", default)]
    pub(crate) sec_filings: BufferedWireValue<SecFilingsNode>,
}

#[derive(Deserialize)]
pub struct SecFilingsNode {
    #[serde(default)]
    pub(crate) filings: BufferedWireValue<Vec<BufferedWireValue<FilingNode>>>,
}

#[derive(Deserialize)]
pub struct FilingNode {
    #[serde(default)]
    pub(crate) date: WireValue<String>,
    #[serde(rename = "epochDate", default)]
    pub(crate) epoch_date: WireValue<i64>,
    #[serde(rename = "type", default)]
    pub(crate) filing_type: WireValue<String>,
    #[serde(default)]
    pub(crate) title: WireValue<String>,
    #[serde(rename = "edgarUrl", default)]
    pub(crate) edgar_url: WireValue<String>,
    #[serde(default)]
    pub(crate) exhibits: BufferedWireValue<Vec<BufferedWireValue<ExhibitNode>>>,
}

#[derive(Deserialize)]
pub struct ExhibitNode {
    #[serde(rename = "type", default)]
    pub(crate) exhibit_type: WireValue<String>,
    #[serde(default)]
    pub(crate) url: WireValue<String>,
    #[serde(rename = "downloadUrl", default)]
    pub(crate) download_url: WireValue<String>,
}
