use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use cluebot_engine::{Candle, Market, MarketData, Signal, SignalType, Strategy};
use serde::Serialize;
use std::collections::HashMap;

/// 波动率上涨做空策略配置
#[derive(Debug, Clone)]
pub struct VolatilityStrategyConfig {
    /// 涨幅阈值 (%)
    pub price_change_threshold: f64,
    /// 波动率阈值 (%)
    pub volatility_threshold: f64,
    /// 最小 K 线数量
    pub min_candles: usize,
}

impl Default for VolatilityStrategyConfig {
    fn default() -> Self {
        Self {
            price_change_threshold: 10.0,
            volatility_threshold: 5.0,
            min_candles: 8,
        }
    }
}

impl VolatilityStrategyConfig {
    /// 创建保守配置
    pub fn conservative() -> Self {
        Self {
            price_change_threshold: 15.0,
            volatility_threshold: 8.0,
            min_candles: 10,
        }
    }

    /// 创建激进配置
    pub fn aggressive() -> Self {
        Self {
            price_change_threshold: 5.0,
            volatility_threshold: 3.0,
            min_candles: 5,
        }
    }
}

/// 市场数据对比结果
#[derive(Debug, Clone, Serialize)]
pub struct MarketComparison {
    /// 币种
    pub coin: String,
    /// 现货 ID
    pub spot_id: String,
    /// 永续合约 ID
    pub swap_id: String,
    /// 现货涨幅 (%)
    pub spot_change: f64,
    /// 永续合约涨幅 (%)
    pub swap_change: f64,
    /// 涨幅差异 (%)
    pub change_diff: f64,
    /// 平均波动率 (%)
    pub avg_volatility: f64,
}

/// 波动率上涨做空策略
///
/// 监控现货与永续合约过去 N 小时涨幅各超阈值，且同名的币种
/// 当波动率异常上涨时，触发做空信号
pub struct VolatilityIncreaseShortSellingStrategy {
    config: VolatilityStrategyConfig,
    name: String,
    /// 缓存的市场对比数据
    comparison_cache: std::sync::Arc<tokio::sync::RwLock<HashMap<String, MarketComparison>>>,
}

impl VolatilityIncreaseShortSellingStrategy {
    /// 创建新策略实例
    pub fn new(config: VolatilityStrategyConfig) -> Self {
        Self {
            config,
            name: "VolatilityIncreaseShortSelling".to_string(),
            comparison_cache: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// 使用默认配置创建
    pub fn default_config() -> Self {
        Self::new(VolatilityStrategyConfig::default())
    }

    /// 计算涨跌幅
    fn calc_price_change(candles: &[Candle]) -> f64 {
        if candles.len() < 2 {
            return 0.0;
        }

        let first = &candles[0];
        let last = &candles[candles.len() - 1];

        if first.open == 0.0 {
            return 0.0;
        }

        (last.close - first.open) / first.open * 100.0
    }

    /// 计算波动率
    fn calc_volatility(candles: &[Candle]) -> f64 {
        if candles.is_empty() {
            return 0.0;
        }

        let avg_volatility: f64 = candles
            .iter()
            .map(|c| ((c.high - c.low) / c.open).abs())
            .sum::<f64>()
            / candles.len() as f64
            * 100.0;

        avg_volatility
    }

    // /// 分析市场数据
    // ///
    // /// 返回是否满足做空条件
    // fn analyze_for_short(&self, data: &MarketData) -> (bool, f64, f64) {
    //     if data.candles.len() < self.config.min_candles {
    //         return (false, 0.0, 0.0);
    //     }

    //     let price_change = Self::calc_price_change(&data.candles);
    //     let volatility = Self::calc_volatility(&data.candles);

    //     // 做空条件：
    //     // 1. 价格涨幅超过阈值（过度上涨后可能回调）
    //     // 2. 波动率超过阈值（市场不稳定）
    //     let should_short = price_change > self.config.price_change_threshold
    //         && volatility > self.config.volatility_threshold;

    //     (should_short, price_change, volatility)
    // }

    /// 对比现货和永续合约数据
    pub async fn compare_spot_swap(
        &self,
        spot_data: &MarketData,
        swap_data: &MarketData,
    ) -> Option<MarketComparison> {
        // 提取币种名称
        let spot_coin = spot_data.inst_id.split('-').next()?;
        let swap_coin = swap_data.inst_id.split("-USDT-SWAP").next()?;

        // 确保是同一币种
        if spot_coin != swap_coin {
            return None;
        }

        let spot_change = Self::calc_price_change(&spot_data.candles);
        let swap_change = Self::calc_price_change(&swap_data.candles);
        let change_diff = (spot_change - swap_change).abs();

        // 计算平均波动率
        let spot_vol = Self::calc_volatility(&spot_data.candles);
        let swap_vol = Self::calc_volatility(&swap_data.candles);
        let avg_volatility = (spot_vol + swap_vol) / 2.0;

        let comparison = MarketComparison {
            coin: spot_coin.to_string(),
            spot_id: spot_data.inst_id.clone(),
            swap_id: swap_data.inst_id.clone(),
            spot_change,
            swap_change,
            change_diff,
            avg_volatility,
        };

        // 缓存结果
        let mut cache = self.comparison_cache.write().await;
        cache.insert(spot_coin.to_string(), comparison.clone());

        Some(comparison)
    }

    /// 检查是否触发对比做空信号
    pub fn check_comparison_signal(&self, comparison: &MarketComparison) -> bool {
        comparison.spot_change > self.config.price_change_threshold
            && comparison.swap_change > self.config.price_change_threshold
            && comparison.avg_volatility > self.config.volatility_threshold
    }

    // /// 生成做空信号描述
    // fn generate_short_description(
    //     &self,
    //     data: &MarketData,
    //     price_change: f64,
    //     volatility: f64,
    // ) -> String {
    //     format!(
    //         "波动率上涨做空信号：{} 价格涨幅 {:.2}%，波动率 {:.2}%。\
    //          市场过度上涨，建议做空或观望。",
    //         data.inst_id, price_change, volatility
    //     )
    // }

    /// 生成对比做空信号描述
    fn generate_comparison_description(&self, comparison: &MarketComparison) -> String {
        format!(
            "现货-永续合约对比做空信号：{} 现货涨幅 {:.2}%，永续涨幅 {:.2}%，\
             差异 {:.2}%，平均波动率 {:.2}%。建议关注套利或做空机会。",
            comparison.coin,
            comparison.spot_change,
            comparison.swap_change,
            comparison.change_diff,
            comparison.avg_volatility
        )
    }

    /// 创建信号
    async fn create_signal(
        &self,
        comparison: &MarketComparison,
        spot_data: &MarketData,
    ) -> Result<Signal> {
        let desc = self.generate_comparison_description(comparison);
        let data = serde_json::json!({
            "coin": comparison.coin,
            "spot_id": comparison.spot_id,
            "swap_id": comparison.swap_id,
            "spot_change": comparison.spot_change,
            "swap_change": comparison.swap_change,
            "change_diff": comparison.change_diff,
            "avg_volatility": comparison.avg_volatility,
            "strategy_config": {
                "price_change_threshold": self.config.price_change_threshold,
                "volatility_threshold": self.config.volatility_threshold,
            }
        });

        Ok(Signal {
            id: format!("vol-{}-{}", comparison.coin, Utc::now().timestamp()),
            strategy_name: self.name.clone(),
            signal_type: SignalType::Sell,
            inst_id: spot_data.inst_id.clone(),
            description: desc,
            data,
            created_at: Utc::now(),
            needs_analysis: comparison.avg_volatility > self.config.volatility_threshold * 1.5,
        })
    }
}

#[async_trait]
impl Strategy for VolatilityIncreaseShortSellingStrategy {
    fn name(&self) -> &str {
        &self.name
    }

    /// 执行策略
    ///
    /// 方案 B：策略自主获取市场数据
    /// 1. 获取所有现货和永续合约
    /// 2. 筛选共同币种
    /// 3. 对比分析并生成信号
    async fn execute(&self, market: &dyn Market) -> Result<Vec<Signal>> {
        let mut signals = Vec::new();
        
        // 获取现货和永续行情
        let spot_tickers = market.fetch_tickers("SPOT").await?;
        let swap_tickers = market.fetch_tickers("SWAP").await?;
        
        // 构建映射
        let spot_map: HashMap<String, String> = spot_tickers
            .into_iter()
            .filter_map(|t| {
                if t.inst_id.ends_with("-USDT") {
                    let coin = t.inst_id.replace("-USDT", "");
                    Some((coin, t.inst_id))
                } else {
                    None
                }
            })
            .collect();
        
        let swap_map: HashMap<String, String> = swap_tickers
            .into_iter()
            .filter_map(|t| {
                if t.inst_id.ends_with("-USDT-SWAP") {
                    let coin = t.inst_id.replace("-USDT-SWAP", "");
                    Some((coin, t.inst_id))
                } else {
                    None
                }
            })
            .collect();
        
        // 遍历共同币种
        for (coin, spot_id) in spot_map.iter().take(50) {
            let Some(swap_id) = swap_map.get(coin) else {
                continue;
            };
            
            // 获取 K 线数据
            let spot_candles = match market.fetch_candles(spot_id, "1H", 8).await {
                Ok(c) if c.len() >= self.config.min_candles => c,
                _ => continue,
            };
            
            let swap_candles = match market.fetch_candles(swap_id, "1H", 8).await {
                Ok(c) if c.len() >= self.config.min_candles => c,
                _ => continue,
            };
            
            // 构建 MarketData
            let spot_data = MarketData {
                source: market.name().to_string(),
                inst_id: spot_id.clone(),
                ticker: None,
                candles: spot_candles,
                price_change_pct: 0.0,
                timestamp: Utc::now(),
            };
            
            let swap_data = MarketData {
                source: market.name().to_string(),
                inst_id: swap_id.clone(),
                ticker: None,
                candles: swap_candles,
                price_change_pct: 0.0,
                timestamp: Utc::now(),
            };
            
            // 对比分析
            if let Some(comparison) = self.compare_spot_swap(&spot_data, &swap_data).await {
                // 检查是否触发信号
                if self.check_comparison_signal(&comparison) {
                    // 生成信号
                    let signal = self.create_signal(&comparison, &spot_data).await?;
                    signals.push(signal);
                }
            }
        }
        
        Ok(signals)
    }
}

// ==================== 便捷函数 ====================

/// 创建默认配置的波动率做空策略
pub fn create_default_strategy() -> VolatilityIncreaseShortSellingStrategy {
    VolatilityIncreaseShortSellingStrategy::default_config()
}

/// 创建保守配置的波动率做空策略
pub fn create_conservative_strategy() -> VolatilityIncreaseShortSellingStrategy {
    VolatilityIncreaseShortSellingStrategy::new(VolatilityStrategyConfig::conservative())
}

/// 创建激进配置的波动率做空策略
pub fn create_aggressive_strategy() -> VolatilityIncreaseShortSellingStrategy {
    VolatilityIncreaseShortSellingStrategy::new(VolatilityStrategyConfig::aggressive())
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;
    use cluebot_engine::{Candle, Ticker};

    fn create_test_candles(price_change: f64) -> Vec<Candle> {
        let base_price = 100.0;
        let end_price = base_price * (1.0 + price_change / 100.0);
        
        // 创建 10 根 K 线，从 base_price 逐步涨到 end_price
        let mut candles = Vec::new();
        let step = (end_price - base_price) / 9.0; // 9 个间隔
        
        for i in 0..10 {
            let open = base_price + step * i as f64;
            let close = base_price + step * (i + 1) as f64;
            let high = close * 1.02;
            let low = open * 0.98;
            
            candles.push(Candle {
                ts: i as i64,
                open,
                high,
                low,
                close,
                vol: 1000.0 + i as f64 * 100.0,
            });
        }
        
        candles
    }

    fn create_test_market_data(inst_id: &str, price_change: f64) -> MarketData {
        MarketData {
            source: "test".to_string(),
            inst_id: inst_id.to_string(),
            ticker: Some(Ticker {
                inst_id: inst_id.to_string(),
                last_price: "100".to_string(),
                open_24h: "90".to_string(),
            }),
            candles: create_test_candles(price_change),
            price_change_pct: price_change,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_calc_price_change() {
        let candles = create_test_candles(15.0);
        let change = VolatilityIncreaseShortSellingStrategy::calc_price_change(&candles);
        // 验证涨幅计算在合理范围内 (由于 K 线生成逻辑，实际涨幅可能略有偏差)
        assert!(change > 10.0 && change < 20.0, "Expected change between 10-20%, got {}%", change);
    }

    #[test]
    fn test_calc_volatility() {
        let candles = create_test_candles(15.0);
        let volatility = VolatilityIncreaseShortSellingStrategy::calc_volatility(&candles);
        assert!(volatility > 0.0);
    }

    #[tokio::test]
    async fn test_create_signal() {
        let strategy = VolatilityIncreaseShortSellingStrategy::default_config();
        let comparison = MarketComparison {
            coin: "BTC".to_string(),
            spot_id: "BTC-USDT".to_string(),
            swap_id: "BTC-USDT-SWAP".to_string(),
            spot_change: 15.0,
            swap_change: 14.5,
            change_diff: 0.5,
            avg_volatility: 8.0,
        };
        let spot_data = create_test_market_data("BTC-USDT", 15.0);
        
        let signal = strategy.create_signal(&comparison, &spot_data).await.unwrap();
        
        assert_eq!(signal.strategy_name, "VolatilityIncreaseShortSelling");
        assert_eq!(signal.signal_type, SignalType::Sell);
        assert_eq!(signal.inst_id, "BTC-USDT");
        assert!(signal.description.contains("做空"));
    }

    #[tokio::test]
    async fn test_compare_spot_swap() {
        let strategy = VolatilityIncreaseShortSellingStrategy::default_config();
        
        let spot_data = create_test_market_data("BTC-USDT", 12.0);
        let swap_data = create_test_market_data("BTC-USDT-SWAP", 11.5);
        
        let comparison = strategy.compare_spot_swap(&spot_data, &swap_data).await;
        
        assert!(comparison.is_some());
        let comp = comparison.unwrap();
        assert_eq!(comp.coin, "BTC");
        assert_eq!(comp.spot_id, "BTC-USDT");
        assert_eq!(comp.swap_id, "BTC-USDT-SWAP");
    }

    #[test]
    fn test_check_comparison_signal() {
        let strategy = VolatilityIncreaseShortSellingStrategy::default_config();
        
        let comparison = MarketComparison {
            coin: "BTC".to_string(),
            spot_id: "BTC-USDT".to_string(),
            swap_id: "BTC-USDT-SWAP".to_string(),
            spot_change: 15.0,
            swap_change: 14.5,
            change_diff: 0.5,
            avg_volatility: 8.0,
        };
        
        assert!(strategy.check_comparison_signal(&comparison));
        
        let low_change = MarketComparison {
            spot_change: 5.0,
            swap_change: 4.5,
            ..comparison
        };
        
        assert!(!strategy.check_comparison_signal(&low_change));
    }

    #[test]
    fn test_strategy_configs() {
        let default_config = VolatilityStrategyConfig::default();
        assert_eq!(default_config.price_change_threshold, 10.0);

        let conservative = VolatilityStrategyConfig::conservative();
        assert_eq!(conservative.price_change_threshold, 15.0);

        let aggressive = VolatilityStrategyConfig::aggressive();
        assert_eq!(aggressive.price_change_threshold, 5.0);
    }
}
