//! 组合汇总：把逐股持仓收益按币种合计。
//!
//! 不同币种一律分开统计——本应用没有汇率数据源，把港币和人民币加在一起
//! 只会得出一个看似精确、实则错误的数字。

use rust_decimal::Decimal;
use serde::Serialize;

use crate::{QuoteSnapshot, StockConfig, calculate_position};

/// 单只股票的持仓收益快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionRow {
    pub symbol: String,
    pub short_name: String,
    pub currency: String,
    pub market_value: Decimal,
    pub cost_basis: Decimal,
    pub unrealized_profit: Decimal,
    pub return_percent: Decimal,
}

/// 单一币种的合计。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioTotal {
    pub currency: String,
    pub market_value: Decimal,
    pub cost_basis: Decimal,
    pub unrealized_profit: Decimal,
    pub return_percent: Decimal,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioSummary {
    /// 已配置持仓且当前有行情的股票，按配置顺序排列。
    pub rows: Vec<PositionRow>,
    /// 按币种分组的合计，币种顺序跟随首次出现的股票。
    pub totals: Vec<PortfolioTotal>,
    /// 配了持仓但暂时取不到行情的股票数量：合计只统计得出来的部分，
    /// 这个数字用于如实告诉用户「合计还差几只」。
    pub missing_quotes: usize,
}

/// 币种标识统一成去空格大写，避免「hkd」与「HKD」被当成两种币。
fn currency_key(currency: &str) -> String {
    let trimmed = currency.trim();
    if trimmed.is_empty() {
        "—".to_owned()
    } else {
        trimmed.to_ascii_uppercase()
    }
}

fn percent_of(profit: Decimal, cost_basis: Decimal) -> Decimal {
    if cost_basis.is_zero() {
        Decimal::ZERO
    } else {
        profit / cost_basis * Decimal::ONE_HUNDRED
    }
}

/// 汇总全部已配置持仓的股票；`quote_of` 按代码取当前行情，取不到即计入 missing。
pub fn summarize_portfolio<'a, F>(stocks: &'a [StockConfig], quote_of: F) -> PortfolioSummary
where
    F: Fn(&str) -> Option<&'a QuoteSnapshot>,
{
    let mut rows = Vec::new();
    let mut totals: Vec<PortfolioTotal> = Vec::new();
    let mut missing_quotes = 0_usize;

    for stock in stocks {
        let Some(position) = stock.position.as_ref() else {
            continue;
        };
        let Some(quote) = quote_of(stock.symbol.trim()) else {
            missing_quotes += 1;
            continue;
        };
        // 成本为零等算不出的情况按「未计入」处理，不污染合计。
        let Ok(pnl) = calculate_position(quote, position) else {
            missing_quotes += 1;
            continue;
        };

        let currency = currency_key(&stock.currency);
        rows.push(PositionRow {
            symbol: stock.symbol.trim().to_owned(),
            short_name: stock.short_name.trim().to_owned(),
            currency: currency.clone(),
            market_value: pnl.market_value,
            cost_basis: pnl.cost_basis,
            unrealized_profit: pnl.unrealized_profit,
            return_percent: pnl.return_percent,
        });

        match totals.iter_mut().find(|total| total.currency == currency) {
            Some(total) => {
                total.market_value += pnl.market_value;
                total.cost_basis += pnl.cost_basis;
                total.unrealized_profit += pnl.unrealized_profit;
            }
            None => totals.push(PortfolioTotal {
                currency,
                market_value: pnl.market_value,
                cost_basis: pnl.cost_basis,
                unrealized_profit: pnl.unrealized_profit,
                return_percent: Decimal::ZERO,
            }),
        }
    }

    // 合计收益率按「合计收益 ÷ 合计成本」重算，而不是各股收益率的平均。
    for total in &mut totals {
        total.return_percent = percent_of(total.unrealized_profit, total.cost_basis);
    }

    PortfolioSummary {
        rows,
        totals,
        missing_quotes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Position;

    fn stock(symbol: &str, currency: &str, position: Option<(&str, &str)>) -> StockConfig {
        StockConfig {
            symbol: symbol.into(),
            short_name: symbol.into(),
            currency: currency.into(),
            position: position.map(|(quantity, cost)| {
                Position::try_new(quantity, cost).expect("fixture position")
            }),
        }
    }

    fn quote(last_price: &str) -> QuoteSnapshot {
        QuoteSnapshot::try_new(last_price, "10", "CNY", 0).expect("fixture quote")
    }

    #[test]
    fn sums_positions_of_the_same_currency() {
        let stocks = vec![
            stock("600756.SH", "CNY", Some(("100", "10"))),
            stock("000001.SZ", "CNY", Some(("200", "5"))),
        ];
        let quotes = [("600756.SH", quote("12")), ("000001.SZ", quote("4"))];

        let summary = summarize_portfolio(&stocks, |symbol| {
            quotes
                .iter()
                .find(|(key, _)| *key == symbol)
                .map(|(_, value)| value)
        });

        assert_eq!(summary.rows.len(), 2);
        assert_eq!(summary.missing_quotes, 0);
        assert_eq!(summary.totals.len(), 1);
        let total = &summary.totals[0];
        assert_eq!(total.currency, "CNY");
        // 成本 100*10 + 200*5 = 2000，市值 100*12 + 200*4 = 2000，收益 0
        assert_eq!(total.cost_basis, Decimal::from(2_000));
        assert_eq!(total.market_value, Decimal::from(2_000));
        assert_eq!(total.unrealized_profit, Decimal::ZERO);
        assert_eq!(total.return_percent, Decimal::ZERO);
    }

    #[test]
    fn never_mixes_different_currencies_into_one_total() {
        let stocks = vec![
            stock("01810.HK", "HKD", Some(("100", "20"))),
            stock("600756.SH", "cny", Some(("100", "10"))),
        ];
        let quotes = [("01810.HK", quote("30")), ("600756.SH", quote("11"))];

        let summary = summarize_portfolio(&stocks, |symbol| {
            quotes
                .iter()
                .find(|(key, _)| *key == symbol)
                .map(|(_, value)| value)
        });

        assert_eq!(summary.totals.len(), 2);
        assert_eq!(summary.totals[0].currency, "HKD");
        assert_eq!(summary.totals[0].unrealized_profit, Decimal::from(1_000));
        // 小写币种归一化到大写，不会另开一组
        assert_eq!(summary.totals[1].currency, "CNY");
        assert_eq!(summary.totals[1].unrealized_profit, Decimal::from(100));
    }

    #[test]
    fn skips_stocks_without_position_and_counts_missing_quotes() {
        let stocks = vec![
            stock("600756.SH", "CNY", None),
            stock("000001.SZ", "CNY", Some(("100", "10"))),
        ];

        let summary = summarize_portfolio(&stocks, |_| None);

        // 没配持仓的股票不算「缺行情」，只有配了持仓却取不到行情才算
        assert!(summary.rows.is_empty());
        assert!(summary.totals.is_empty());
        assert_eq!(summary.missing_quotes, 1);
    }

    #[test]
    fn total_return_uses_aggregate_cost_not_the_average_of_rates() {
        // 大仓小涨 + 小仓大涨：按仓位加权的合计收益率必须贴近大仓
        let stocks = vec![
            stock("600756.SH", "CNY", Some(("1000", "10"))),
            stock("000001.SZ", "CNY", Some(("10", "10"))),
        ];
        let quotes = [("600756.SH", quote("11")), ("000001.SZ", quote("20"))];

        let summary = summarize_portfolio(&stocks, |symbol| {
            quotes
                .iter()
                .find(|(key, _)| *key == symbol)
                .map(|(_, value)| value)
        });

        let total = &summary.totals[0];
        // 成本 10000+100=10100，收益 1000+100=1100 → 10.89%，而非 (10%+100%)/2
        assert_eq!(total.cost_basis, Decimal::from(10_100));
        assert_eq!(total.unrealized_profit, Decimal::from(1_100));
        assert_eq!(total.return_percent.round_dp(2), "10.89".parse().unwrap());
    }
}
