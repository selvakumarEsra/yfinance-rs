//! Calendars (REQ-YGAP-008) and per-symbol earnings dates (REQ-YGAP-004).
//!
//! NOTE: every `calendars_api_visualization_*` and `earnings_dates_api_visualization_*`
//! fixture is SYNTHETIC. Yahoo rate-limited the recording machine, so the shapes were
//! modelled on Python `yfinance` (`calendars.py`, `base.py`) parsers. Re-record live.

mod common;

#[path = "calendars/earnings_dates.rs"]
mod calendars_earnings_dates;
#[path = "calendars/live.rs"]
mod calendars_live;
#[path = "calendars/offline.rs"]
mod calendars_offline;
#[path = "calendars/support.rs"]
mod calendars_support;
