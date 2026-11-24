//! Core domain library for TickerBar.

mod alerts;
mod config;
mod domain;
mod portfolio;
mod provider;
mod quick_view;
mod runtime;
mod storage;

pub use alerts::{AlertComparator, AlertMetric, AlertNotification, AlertRepeat, AlertRule};
pub use config::{
    AppConfig, CONFIG_SCHEMA_VERSION, DisplayPreset, MAX_STOCKS, ProviderKind, QuickViewSort,
    StockConfig, apply_display_preset,
};
pub use domain::{
    CompactStyle, ConnectionState, DisplayConfig, DisplayMetric, DomainError, MetricConfig,
    Position, PositionPnl, QuoteSnapshot, calculate_position, render_tray_title,
    render_tray_title_with_portfolio,
};
pub use portfolio::{PortfolioSummary, PortfolioTotal, PositionRow, summarize_portfolio};
pub use provider::{
    LongbridgeQuoteProvider, MockQuoteProvider, NetworkFailure, ProviderError, ProviderUpdate,
    QuoteProvider, StockSearchResult, TencentQuoteProvider,
};
pub use quick_view::{QuickViewRow, QuickViewSnapshot, build_quick_view_snapshot};
pub use storage::{StorageError, load_config, save_config};

pub use runtime::run;
