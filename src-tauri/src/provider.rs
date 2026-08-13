use std::{error::Error, fmt, time::Duration};

use rust_decimal::Decimal;
use serde::Serialize;
use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset, Weekday};

use crate::{ConnectionState, DomainError, QuoteSnapshot, StockConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderUpdate {
    pub quote: QuoteSnapshot,
    pub connection: ConnectionState,
}

pub trait QuoteProvider {
    fn next_quote(
        &mut self,
        stock: &StockConfig,
        timestamp: i64,
        updated_time: &str,
    ) -> Result<ProviderUpdate, ProviderError>;
}

#[derive(Debug, Default)]
pub struct MockQuoteProvider {
    tick: usize,
}

impl QuoteProvider for MockQuoteProvider {
    fn next_quote(
        &mut self,
        stock: &StockConfig,
        timestamp: i64,
        updated_time: &str,
    ) -> Result<ProviderUpdate, ProviderError> {
        const OFFSETS_IN_CENTS: [i64; 16] = [
            65, 68, 72, 77, 81, 86, 90, 87, 83, 79, 74, 70, 66, 63, 61, 64,
        ];
        let cents = 4_220 + OFFSETS_IN_CENTS[self.tick % OFFSETS_IN_CENTS.len()];
        self.tick = self.tick.wrapping_add(1);

        let last_price = Decimal::new(cents, 2).to_string();
        let quote = QuoteSnapshot::try_new(&last_price, "42.20", &stock.currency, timestamp)?
            .with_identity(&stock.symbol, &stock.short_name)
            .with_day_range("43.10", "41.90")?
            .with_updated_time(updated_time);

        Ok(ProviderUpdate {
            quote,
            connection: ConnectionState::Live,
        })
    }
}

#[derive(Debug, Default)]
pub struct LongbridgeQuoteProvider;

impl QuoteProvider for LongbridgeQuoteProvider {
    fn next_quote(
        &mut self,
        _stock: &StockConfig,
        _timestamp: i64,
        _updated_time: &str,
    ) -> Result<ProviderUpdate, ProviderError> {
        Err(ProviderError::NotConfigured)
    }
}

const TENCENT_QUOTE_ENDPOINT: &str = "https://qt.gtimg.cn/q=";
const TENCENT_SEARCH_ENDPOINT: &str = "https://smartbox.gtimg.cn/s3/";
const MAX_SEARCH_QUERY_CHARS: usize = 32;
/// 正常行情/搜索响应都在几 KB 内；超过上限视为异常响应，避免长驻进程反复吞入大包体。
const MAX_RESPONSE_BYTES: usize = 64 * 1024;
/// 交易时段内行情时间落后当前时间超过该秒数时，标记为延迟而非实时。
const MAX_QUOTE_STALENESS_SECS: i64 = 300;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StockSearchResult {
    pub symbol: String,
    pub name: String,
    pub market: String,
    pub currency: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TencentMarket {
    Shanghai,
    Shenzhen,
    Beijing,
    HongKong,
}

impl TencentMarket {
    fn from_code(value: &str) -> Option<Self> {
        match value {
            "SH" | "SS" => Some(Self::Shanghai),
            "SZ" => Some(Self::Shenzhen),
            "BJ" => Some(Self::Beijing),
            "HK" | "R_HK" => Some(Self::HongKong),
            _ => None,
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Self::Shanghai => "SH",
            Self::Shenzhen => "SZ",
            Self::Beijing => "BJ",
            Self::HongKong => "HK",
        }
    }

    fn provider_prefix(self) -> &'static str {
        match self {
            Self::Shanghai => "sh",
            Self::Shenzhen => "sz",
            Self::Beijing => "bj",
            Self::HongKong => "r_hk",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Shanghai => "沪市",
            Self::Shenzhen => "深市",
            Self::Beijing => "北交所",
            Self::HongKong => "港股",
        }
    }

    fn currency(self) -> &'static str {
        match self {
            Self::HongKong => "HKD",
            Self::Shanghai | Self::Shenzhen | Self::Beijing => "CNY",
        }
    }

    /// 连续竞价时段（交易所当地时间 UTC+8，自午夜起的分钟数）。
    fn trading_sessions(self) -> [(u16, u16); 2] {
        match self {
            Self::Shanghai | Self::Shenzhen | Self::Beijing => [(570, 690), (780, 900)],
            Self::HongKong => [(570, 720), (780, 960)],
        }
    }
}

fn exchange_offset() -> UtcOffset {
    UtcOffset::from_hms(8, 0, 0).unwrap_or(UtcOffset::UTC)
}

fn parse_quote_datetime(raw: &str, offset: UtcOffset) -> Option<OffsetDateTime> {
    let (date_part, clock_part) = raw.trim().split_once(char::is_whitespace)?;
    let mut date_fields = date_part.split('/');
    let year: i32 = date_fields.next()?.parse().ok()?;
    let month: u8 = date_fields.next()?.parse().ok()?;
    let day: u8 = date_fields.next()?.parse().ok()?;
    let mut clock_fields = clock_part.split(':');
    let hour: u8 = clock_fields.next()?.parse().ok()?;
    let minute: u8 = clock_fields.next()?.parse().ok()?;
    let second: u8 = clock_fields.next().and_then(|value| value.parse().ok())?;

    let date = Date::from_calendar_date(year, Month::try_from(month).ok()?, day).ok()?;
    let time = Time::from_hms(hour, minute, second).ok()?;
    Some(PrimitiveDateTime::new(date, time).assume_offset(offset))
}

/// 依据交易时段与行情时间戳推导市场状态：
/// 休市（含午间与收盘后）→ Closed；行情停留在往日（节假日）→ Closed；
/// 交易时段内行情明显滞后 → Delayed；其余 → Live。
fn derive_market_state(
    market: TencentMarket,
    now_unix: i64,
    quote_datetime: &str,
) -> ConnectionState {
    let offset = exchange_offset();
    let Ok(now) = OffsetDateTime::from_unix_timestamp(now_unix) else {
        return ConnectionState::Live;
    };
    let now = now.to_offset(offset);

    if matches!(now.weekday(), Weekday::Saturday | Weekday::Sunday) {
        return ConnectionState::Closed;
    }
    let minutes = u16::from(now.hour()) * 60 + u16::from(now.minute());
    let in_session = market
        .trading_sessions()
        .iter()
        .any(|(start, end)| minutes >= *start && minutes < *end);
    if !in_session {
        return ConnectionState::Closed;
    }

    match parse_quote_datetime(quote_datetime, offset) {
        Some(quote_time) if quote_time.date() != now.date() => ConnectionState::Closed,
        Some(quote_time)
            if now.unix_timestamp() - quote_time.unix_timestamp() > MAX_QUOTE_STALENESS_SECS =>
        {
            ConnectionState::Delayed
        }
        // 时间戳缺失或异常时不冒充延迟/休市，按实时处理。
        _ => ConnectionState::Live,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedSymbol {
    market: TencentMarket,
    code: String,
}

impl NormalizedSymbol {
    fn parse(symbol: &str) -> Result<Self, ProviderError> {
        let normalized = symbol.trim().to_ascii_uppercase();
        let parsed = if let Some((code, market)) = normalized.rsplit_once('.') {
            TencentMarket::from_code(market).map(|market| (market, code))
        } else {
            ["R_HK", "SH", "SS", "SZ", "BJ", "HK"]
                .into_iter()
                .find_map(|prefix| {
                    TencentMarket::from_code(prefix).zip(normalized.strip_prefix(prefix))
                })
        };
        let (market, code) =
            parsed.ok_or_else(|| ProviderError::UnsupportedSymbol(symbol.to_owned()))?;
        if code.is_empty() || !code.bytes().all(|character| character.is_ascii_digit()) {
            return Err(ProviderError::UnsupportedSymbol(symbol.to_owned()));
        }

        let code = match market {
            TencentMarket::HongKong if code.len() <= 5 => format!("{code:0>5}"),
            TencentMarket::Shanghai | TencentMarket::Shenzhen | TencentMarket::Beijing
                if code.len() == 6 =>
            {
                code.to_owned()
            }
            _ => return Err(ProviderError::UnsupportedSymbol(symbol.to_owned())),
        };
        Ok(Self { market, code })
    }

    fn from_search_fields(market: &str, code: &str) -> Result<Self, ProviderError> {
        let market = TencentMarket::from_code(&market.to_ascii_uppercase())
            .ok_or_else(|| ProviderError::UnsupportedSymbol(format!("{market}{code}")))?;
        Self::parse(&format!("{code}.{}", market.suffix()))
    }

    fn provider_symbol(&self) -> String {
        format!("{}{}", self.market.provider_prefix(), self.code)
    }

    fn canonical_symbol(&self) -> String {
        format!("{}.{}", self.code, self.market.suffix())
    }
}

async fn read_limited_body(mut response: reqwest::Response) -> Result<String, ProviderError> {
    if let Some(length) = response.content_length()
        && length > MAX_RESPONSE_BYTES as u64
    {
        return Err(ProviderError::ResponseTooLarge(length as usize));
    }
    let mut body: Vec<u8> = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| ProviderError::Network(error.to_string()))?
    {
        if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(ProviderError::ResponseTooLarge(body.len() + chunk.len()));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

#[derive(Debug, Clone)]
pub struct TencentQuoteProvider {
    client: reqwest::Client,
}

impl TencentQuoteProvider {
    pub fn new() -> Result<Self, ProviderError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .user_agent(concat!("TickerBar/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| ProviderError::Network(error.to_string()))?;
        Ok(Self { client })
    }

    /// 批量拉取行情：一次请求带上全部股票代码，返回与入参逐一对齐的结果。
    /// 外层 Err 表示整次请求失败（网络/HTTP/超限）；内层 Err 只影响单只股票。
    pub async fn next_quotes(
        &self,
        stocks: &[StockConfig],
        timestamp: i64,
    ) -> Result<Vec<Result<ProviderUpdate, ProviderError>>, ProviderError> {
        if stocks.is_empty() {
            return Ok(Vec::new());
        }
        let provider_symbols: Vec<String> = stocks
            .iter()
            .map(|stock| Self::normalize_symbol(&stock.symbol))
            .collect::<Result<_, _>>()?;
        let response = self
            .client
            .get(format!(
                "{TENCENT_QUOTE_ENDPOINT}{}",
                provider_symbols.join(",")
            ))
            .send()
            .await
            .map_err(|error| ProviderError::Network(error.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(ProviderError::HttpStatus(status.as_u16()));
        }
        let body = read_limited_body(response).await?;
        Ok(Self::parse_batch_response(
            stocks,
            &provider_symbols,
            timestamp,
            &body,
        ))
    }

    pub async fn search_stocks(
        &self,
        query: &str,
    ) -> Result<Vec<StockSearchResult>, ProviderError> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        if query.chars().count() > MAX_SEARCH_QUERY_CHARS {
            return Err(ProviderError::InvalidSearchQuery);
        }

        let mut url = reqwest::Url::parse(TENCENT_SEARCH_ENDPOINT)
            .map_err(|error| ProviderError::Network(error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("v", "2")
            .append_pair("t", "all")
            .append_pair("c", "1")
            .append_pair("q", query);
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| ProviderError::Network(error.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(ProviderError::HttpStatus(status.as_u16()));
        }
        let body = read_limited_body(response).await?;
        Self::parse_search_response(&body)
    }

    pub(crate) fn normalize_symbol(symbol: &str) -> Result<String, ProviderError> {
        Ok(NormalizedSymbol::parse(symbol)?.provider_symbol())
    }

    pub(crate) fn canonical_symbol(symbol: &str) -> Result<String, ProviderError> {
        Ok(NormalizedSymbol::parse(symbol)?.canonical_symbol())
    }

    fn parse_search_response(response: &str) -> Result<Vec<StockSearchResult>, ProviderError> {
        let start = response
            .find('"')
            .ok_or_else(|| ProviderError::InvalidResponse("missing search payload".into()))?;
        let end = response
            .rfind('"')
            .filter(|end| *end > start)
            .ok_or_else(|| ProviderError::InvalidResponse("missing search payload".into()))?;
        let decoded: String = serde_json::from_str(&response[start..=end])
            .map_err(|error| ProviderError::InvalidResponse(error.to_string()))?;

        Ok(decoded
            .split('^')
            .filter_map(|candidate| {
                let fields: Vec<_> = candidate.split('~').collect();
                if fields.len() < 5 || !matches!(fields[4], "GP" | "GP-A") {
                    return None;
                }
                let normalized = NormalizedSymbol::from_search_fields(fields[0], fields[1]).ok()?;
                let name = fields[2].trim();
                if name.is_empty() {
                    return None;
                }
                Some(StockSearchResult {
                    symbol: normalized.canonical_symbol(),
                    name: name.to_owned(),
                    market: normalized.market.label().to_owned(),
                    currency: normalized.market.currency().to_owned(),
                })
            })
            .take(8)
            .collect())
    }

    /// 从整段批量响应中按 `v_<代码>="..."` 提取各股票负载，逐股解析。
    fn parse_batch_response(
        stocks: &[StockConfig],
        provider_symbols: &[String],
        timestamp: i64,
        response: &str,
    ) -> Vec<Result<ProviderUpdate, ProviderError>> {
        let payloads: Vec<(&str, &str)> = response
            .split(';')
            .filter_map(|segment| {
                let rest = segment.trim().strip_prefix("v_")?;
                let (key, remainder) = rest.split_once('=')?;
                let payload = remainder
                    .split_once('"')
                    .and_then(|(_, tail)| tail.split_once('"').map(|(value, _)| value))?;
                Some((key, payload))
            })
            .collect();

        stocks
            .iter()
            .zip(provider_symbols)
            .map(|(stock, provider_symbol)| {
                let payload = payloads
                    .iter()
                    .find(|(key, _)| *key == provider_symbol.as_str())
                    .map(|(_, payload)| *payload)
                    .ok_or_else(|| {
                        ProviderError::InvalidResponse(format!(
                            "no quote payload for {}",
                            stock.symbol
                        ))
                    })?;
                Self::parse_payload(stock, timestamp, payload)
            })
            .collect()
    }

    fn parse_payload(
        stock: &StockConfig,
        timestamp: i64,
        payload: &str,
    ) -> Result<ProviderUpdate, ProviderError> {
        let fields: Vec<_> = payload.split('~').collect();
        if fields.len() < 35 || fields[3].is_empty() || fields[4].is_empty() {
            return Err(ProviderError::InvalidResponse(
                "quote payload is incomplete".into(),
            ));
        }

        let currency = fields
            .get(75)
            .filter(|value| !value.is_empty())
            .copied()
            .unwrap_or(stock.currency.as_str());
        let short_name = if stock.short_name.trim().is_empty() {
            fields[1]
        } else {
            stock.short_name.trim()
        };
        let updated_time = fields[30]
            .split_ascii_whitespace()
            .nth(1)
            .and_then(|clock| clock.get(..5))
            .unwrap_or(fields[30]);
        let quote = QuoteSnapshot::try_new(fields[3], fields[4], currency, timestamp)?
            .with_identity(stock.symbol.trim(), short_name)
            .with_day_range(fields[33], fields[34])?
            .with_updated_time(updated_time);
        let connection = NormalizedSymbol::parse(&stock.symbol)
            .map(|symbol| derive_market_state(symbol.market, timestamp, fields[30]))
            .unwrap_or(ConnectionState::Live);

        Ok(ProviderUpdate { quote, connection })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    NotConfigured,
    UnsupportedSymbol(String),
    InvalidSearchQuery,
    Network(String),
    HttpStatus(u16),
    ResponseTooLarge(usize),
    InvalidResponse(String),
    InvalidQuote(DomainError),
}

impl fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConfigured => write!(formatter, "quote provider is not configured"),
            Self::UnsupportedSymbol(symbol) => {
                write!(formatter, "unsupported stock symbol: {symbol}")
            }
            Self::InvalidSearchQuery => write!(formatter, "股票搜索内容过长"),
            Self::Network(message) => write!(formatter, "quote request failed: {message}"),
            Self::HttpStatus(status) => {
                write!(formatter, "quote request returned HTTP {status}")
            }
            Self::ResponseTooLarge(size) => {
                write!(
                    formatter,
                    "quote response exceeds {MAX_RESPONSE_BYTES} bytes: {size}"
                )
            }
            Self::InvalidResponse(message) => {
                write!(formatter, "quote response is invalid: {message}")
            }
            Self::InvalidQuote(source) => {
                write!(formatter, "provider returned invalid quote: {source}")
            }
        }
    }
}

impl Error for ProviderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidQuote(source) => Some(source),
            Self::NotConfigured
            | Self::UnsupportedSymbol(_)
            | Self::InvalidSearchQuery
            | Self::Network(_)
            | Self::HttpStatus(_)
            | Self::ResponseTooLarge(_)
            | Self::InvalidResponse(_) => None,
        }
    }
}

impl From<DomainError> for ProviderError {
    fn from(value: DomainError) -> Self {
        Self::InvalidQuote(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DisplayPreset, apply_display_preset, render_tray_title};

    const TENCENT_XIAOMI_RESPONSE: &str = r#"v_r_hk01810="100~小米集团-W~01810~27.380~28.780~28.740~102533383.0~0~0~27.380~0~0~0~0~0~0~0~0~0~27.380~0~0~0~0~0~0~0~0~0~102533383.0~2026/08/03 10:00:40~-1.400~-4.86~28.760~27.240~27.380~102533383.0~2836431826.930~0~15.35~~0~0~5.28~5858.9016~7075.5637~XIAOMI-W~0.00~59.900~21.300~3.44~5.35~0~0~0~0~0~17.63~2.40~0.48~200~-30.33~-4.53~GP~13.15~7.03~-1.30~17.61~-11.16~25842088167.00~21398472034.00~33.07~0.000~27.663~-36.80~HKD~1~30";"#;
    const TENCENT_SEARCH_RESPONSE: &str = r#"v_hint="sh~600519~\u8d35\u5dde\u8305\u53f0~gzmt~GP-A^hk~01810~\u5c0f\u7c73\u96c6\u56e2w~xmjtw~GP^hk~13011~\u5c0f\u7c73\u4fe1\u8bc1~xmxz~QZ""#;

    #[test]
    fn normalizes_supported_stock_symbols_for_tencent() {
        assert_eq!(
            TencentQuoteProvider::normalize_symbol("01810.HK").unwrap(),
            "r_hk01810"
        );
        assert_eq!(
            TencentQuoteProvider::normalize_symbol("1810.HK").unwrap(),
            "r_hk01810"
        );
        assert_eq!(
            TencentQuoteProvider::normalize_symbol("600519.SH").unwrap(),
            "sh600519"
        );
        assert_eq!(
            TencentQuoteProvider::canonical_symbol("600519.SS").unwrap(),
            "600519.SH"
        );
        assert_eq!(
            TencentQuoteProvider::normalize_symbol("000001.SZ").unwrap(),
            "sz000001"
        );
        assert_eq!(
            TencentQuoteProvider::normalize_symbol("sh600519").unwrap(),
            "sh600519"
        );
        assert_eq!(
            TencentQuoteProvider::canonical_symbol("sh600519").unwrap(),
            "600519.SH"
        );
    }

    // 2026-08-03（周一）10:00:40 UTC+8，与 fixture 内的行情时间一致。
    const XIAOMI_QUOTE_UNIX: i64 = 1_785_722_440;

    fn parse_single(
        stock: &StockConfig,
        timestamp: i64,
        response: &str,
    ) -> Result<ProviderUpdate, ProviderError> {
        let provider_symbols = vec![TencentQuoteProvider::normalize_symbol(&stock.symbol).unwrap()];
        TencentQuoteProvider::parse_batch_response(
            std::slice::from_ref(stock),
            &provider_symbols,
            timestamp,
            response,
        )
        .remove(0)
    }

    #[test]
    fn parses_a_real_tencent_quote_response() {
        let config = crate::AppConfig::default();
        let mut stock = config.stocks[0].clone();
        stock.position = Some(crate::Position::try_new("250", "39.46").expect("fixture position"));

        let update = parse_single(&stock, XIAOMI_QUOTE_UNIX, TENCENT_XIAOMI_RESPONSE).unwrap();

        assert_eq!(update.quote.last_price.to_string(), "27.380");
        assert_eq!(update.quote.previous_close.to_string(), "28.780");
        assert_eq!(update.quote.day_high.unwrap().to_string(), "28.760");
        assert_eq!(update.quote.day_low.unwrap().to_string(), "27.240");
        assert_eq!(update.quote.currency, "HKD");
        assert_eq!(update.quote.updated_time.as_deref(), Some("10:00"));
        assert_eq!(update.connection, ConnectionState::Live);

        let price_title = render_tray_title(
            &update.quote,
            stock.position.as_ref(),
            &config.display,
            &update.connection,
        )
        .unwrap();
        assert_eq!(price_title, "27.38 ↓4.86%");

        let position_display = crate::DisplayConfig {
            items: apply_display_preset(DisplayPreset::Position),
            ..config.display
        };
        let position_title = render_tray_title(
            &update.quote,
            stock.position.as_ref(),
            &position_display,
            &update.connection,
        )
        .unwrap();
        assert_eq!(position_title, "-3020 ↓30.61%");
    }

    #[test]
    fn rejects_unsupported_symbols_and_malformed_responses() {
        assert!(TencentQuoteProvider::normalize_symbol("../../bad").is_err());
        let stock = crate::AppConfig::default().stocks[0].clone();
        assert!(parse_single(&stock, 0, "broken").is_err());
    }

    #[test]
    fn parses_a_batch_response_and_flags_missing_stocks_individually() {
        let template = crate::AppConfig::default().stocks[0].clone();
        let stocks = vec![
            StockConfig {
                symbol: "01810.HK".into(),
                short_name: "小米".into(),
                currency: "HKD".into(),
                position: None,
            },
            StockConfig {
                symbol: "600519.SH".into(),
                short_name: "贵州茅台".into(),
                currency: "CNY".into(),
                ..template
            },
        ];
        let provider_symbols = vec!["r_hk01810".to_owned(), "sh600519".to_owned()];

        // 响应里只有小米：小米解析成功，茅台单独报错，互不影响。
        let results = TencentQuoteProvider::parse_batch_response(
            &stocks,
            &provider_symbols,
            XIAOMI_QUOTE_UNIX,
            TENCENT_XIAOMI_RESPONSE,
        );

        assert_eq!(results.len(), 2);
        let xiaomi = results[0].as_ref().expect("xiaomi payload should parse");
        assert_eq!(xiaomi.quote.symbol.as_deref(), Some("01810.HK"));
        assert!(matches!(results[1], Err(ProviderError::InvalidResponse(_))));
    }

    #[test]
    fn derives_market_state_from_sessions_and_quote_freshness() {
        const QUOTE_TIME: &str = "2026/08/03 10:00:40";
        let hk = TencentMarket::HongKong;
        let sh = TencentMarket::Shanghai;

        // 交易时段内、行情新鲜 → 实时
        assert_eq!(
            derive_market_state(hk, XIAOMI_QUOTE_UNIX, QUOTE_TIME),
            ConnectionState::Live
        );
        // 行情落后超过 5 分钟 → 延迟
        assert_eq!(
            derive_market_state(hk, XIAOMI_QUOTE_UNIX + 600, QUOTE_TIME),
            ConnectionState::Delayed
        );
        // 当天 20:00 → 收盘
        assert_eq!(
            derive_market_state(hk, XIAOMI_QUOTE_UNIX + 10 * 3_600, QUOTE_TIME),
            ConnectionState::Closed
        );
        // 港股午间休市 12:30 → 休市；同一时间 A 股 12:30 也在午休
        let half_past_noon = XIAOMI_QUOTE_UNIX + 2 * 3_600 + 30 * 60;
        assert_eq!(
            derive_market_state(hk, half_past_noon, QUOTE_TIME),
            ConnectionState::Closed
        );
        assert_eq!(
            derive_market_state(sh, half_past_noon, QUOTE_TIME),
            ConnectionState::Closed
        );
        // A 股 15:30 已收盘，港股 15:30 仍在交易（行情为当日但已滞后 → 延迟）
        let half_past_three = XIAOMI_QUOTE_UNIX + 5 * 3_600 + 30 * 60;
        assert_eq!(
            derive_market_state(sh, half_past_three, QUOTE_TIME),
            ConnectionState::Closed
        );
        assert_eq!(
            derive_market_state(hk, half_past_three, QUOTE_TIME),
            ConnectionState::Delayed
        );
        // 周六 → 休市
        assert_eq!(
            derive_market_state(hk, XIAOMI_QUOTE_UNIX + 5 * 86_400, QUOTE_TIME),
            ConnectionState::Closed
        );
        // 节假日：当前是交易时段但行情停留在往日 → 休市
        assert_eq!(
            derive_market_state(hk, XIAOMI_QUOTE_UNIX + 86_400, QUOTE_TIME),
            ConnectionState::Closed
        );
        // 行情时间无法解析 → 保守按实时处理
        assert_eq!(
            derive_market_state(hk, XIAOMI_QUOTE_UNIX, "garbage"),
            ConnectionState::Live
        );
    }

    #[test]
    fn parses_supported_stock_search_results_and_ignores_warrants() {
        let results = TencentQuoteProvider::parse_search_response(TENCENT_SEARCH_RESPONSE).unwrap();

        assert_eq!(
            results,
            vec![
                StockSearchResult {
                    symbol: "600519.SH".into(),
                    name: "贵州茅台".into(),
                    market: "沪市".into(),
                    currency: "CNY".into(),
                },
                StockSearchResult {
                    symbol: "01810.HK".into(),
                    name: "小米集团w".into(),
                    market: "港股".into(),
                    currency: "HKD".into(),
                },
            ]
        );
    }
}
