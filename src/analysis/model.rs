// Re-export types from paft without using prelude
pub use paft::fundamentals::analysis::{
    EarningsTrendRow, PriceTarget, RecommendationRow, RecommendationSummary, UpgradeDowngradeRow,
};

use chrono::NaiveDate;
use paft::Decimal;
use paft::money::Price;
use serde::{Deserialize, Serialize};

/// One reported quarter from Yahoo's `earningsHistory` quoteSummary module.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EarningsHistoryRow {
    /// Fiscal quarter end date.
    pub quarter: NaiveDate,
    /// Yahoo's relative period label, such as `-1q` (most recent) to `-4q`.
    pub period: Option<String>,
    /// Reported EPS, in the resolved earnings currency.
    pub eps_actual: Option<Price>,
    /// Consensus EPS estimate, in the resolved earnings currency.
    pub eps_estimate: Option<Price>,
    /// Reported minus estimated EPS, in the resolved earnings currency.
    pub eps_difference: Option<Price>,
    /// Surprise as a fraction of the estimate (`0.05` means 5 %).
    pub surprise_percent: Option<Decimal>,
}
