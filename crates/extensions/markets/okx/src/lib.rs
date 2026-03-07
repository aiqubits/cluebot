use anyhow::Result;
use async_trait::async_trait;
use cluebot_engine::{Candle, Market, Ticker};
use serde::Deserialize;

const BASE_URL: &str = "https://www.okx.com";

/// OKX API 响应包装
#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    code: String,
    msg: Option<String>,
    data: Vec<T>,
}

/// OKX Ticker 原始数据
#[derive(Debug, Deserialize)]
struct OkxTicker {
    #[serde(rename = "instId")]
    inst_id: String,
    #[serde(rename = "last")]
    last: String,
    #[serde(rename = "open24h")]
    open_24h: String,
}

/// OKX Candle 原始数据
/// OKX 返回格式: [ts, open, high, low, close, vol, volCcy]
type OkxCandle = Vec<String>;

/// OKX 市场客户端
pub struct OkxMarket {
    client: reqwest::Client,
    name: String,
}

impl OkxMarket {
    /// 创建新的 OKX 市场客户端
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            name: "OKX".to_string(),
        }
    }

    /// 获取所有交易对行情
    async fn fetch_tickers_inner(&self, inst_type: &str) -> Result<Vec<Ticker>> {
        let url = format!("{}/api/v5/market/tickers?instType={}", BASE_URL, inst_type);
        
        let response = self
            .client
            .get(&url)
            .send()
            .await?
            .json::<ApiResponse<OkxTicker>>()
            .await?;

        if response.code != "0" {
            return Err(anyhow::anyhow!(
                "OKX API error: {:?}",
                response.msg
            ));
        }

        let tickers: Vec<Ticker> = response
            .data
            .into_iter()
            .map(|t| Ticker {
                inst_id: t.inst_id,
                last_price: t.last,
                open_24h: t.open_24h,
            })
            .collect();

        Ok(tickers)
    }

    /// 获取 K 线数据
    async fn fetch_candles_inner(
        &self,
        inst_id: &str,
        bar: &str,
        limit: u32,
    ) -> Result<Vec<Candle>> {
        let url = format!(
            "{}/api/v5/market/candles?instId={}&bar={}&limit={}",
            BASE_URL, inst_id, bar, limit
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await?
            .json::<ApiResponse<OkxCandle>>()
            .await?;

        if response.code != "0" {
            return Err(anyhow::anyhow!(
                "OKX API error: {:?}",
                response.msg
            ));
        }

        let candles: Vec<Candle> = response
            .data
            .into_iter()
            .filter_map(|c| Candle::from_okx(&c).ok())
            .collect();

        Ok(candles)
    }

}

impl Default for OkxMarket {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Market for OkxMarket {
    fn name(&self) -> &str {
        &self.name
    }

    async fn fetch_tickers(&self, inst_type: &str) -> Result<Vec<Ticker>> {
        self.fetch_tickers_inner(inst_type).await
    }

    async fn fetch_candles(&self, inst_id: &str, bar: &str, limit: u32) -> Result<Vec<Candle>> {
        self.fetch_candles_inner(inst_id, bar, limit).await
    }
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_okx_market_creation() {
        let market = OkxMarket::new();
        assert_eq!(market.name(), "OKX");
    }

    #[test]
    fn test_candle_from_okx() {
        let data = vec![
            "1640995200000".to_string(), // ts
            "100.0".to_string(),         // open
            "110.0".to_string(),         // high
            "90.0".to_string(),          // low
            "105.0".to_string(),         // close
            "1000.0".to_string(),        // vol
        ];

        let candle = Candle::from_okx(&data).unwrap();
        assert_eq!(candle.ts, 1640995200000);
        assert_eq!(candle.open, 100.0);
        assert_eq!(candle.high, 110.0);
        assert_eq!(candle.low, 90.0);
        assert_eq!(candle.close, 105.0);
        assert_eq!(candle.vol, 1000.0);
    }

    // 注意：以下测试需要网络连接，默认不运行
    // #[tokio::test]
    // async fn test_fetch_tickers() {
    //     let market = OkxMarket::new();
    //     let tickers = market.fetch_tickers("SPOT").await.unwrap();
    //     assert!(!tickers.is_empty());
    // }
}
