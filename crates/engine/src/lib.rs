use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use cluebot_runtime::{LifecycleHandler, RuntimeState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration};

// ==================== Trait 定义 ====================

/// 策略 trait - 定义策略的通用接口
///
/// 所有具体策略需要实现此 trait，被 Engine 调用
#[async_trait]
pub trait Strategy: Send + Sync {
    /// 获取策略名称
    fn name(&self) -> &str;

    /// 检查策略条件
    ///
    /// # Arguments
    /// * `data` - 市场数据
    ///
    /// # Returns
    /// * `Ok(true)` - 条件满足，应生成信号
    /// * `Ok(false)` - 条件不满足
    async fn check_conditions(&self, data: &MarketData) -> Result<bool>;

    /// 生成交易信号
    ///
    /// # Arguments
    /// * `data` - 市场数据
    ///
    /// # Returns
    /// * `Ok(Signal)` - 交易信号
    async fn generate_signal(&self, data: &MarketData) -> Result<Signal>;
}

/// 市场 trait - 定义市场数据获取接口
///
/// OKX、Binance 等交易所实现此 trait
#[async_trait]
pub trait Market: Send + Sync {
    /// 获取市场名称
    fn name(&self) -> &str;

    /// 获取所有交易对行情
    async fn fetch_tickers(&self, inst_type: &str) -> Result<Vec<Ticker>>;

    /// 获取 K 线数据
    ///
    /// # Arguments
    /// * `inst_id` - 交易对 ID，如 "BTC-USDT"
    /// * `bar` - 时间周期，如 "1H"
    /// * `limit` - 返回条数
    async fn fetch_candles(&self, inst_id: &str, bar: &str, limit: u32) -> Result<Vec<Candle>>;
}

/// 通知渠道 trait - 定义通知发送接口
///
/// Lark、Email 等渠道实现此 trait
#[async_trait]
pub trait Channel: Send + Sync {
    /// 获取渠道名称
    fn name(&self) -> &str;

    /// 发送通知
    ///
    /// # Arguments
    /// * `message` - 通知内容
    async fn send(&self, message: &str) -> Result<()>;
}

// ==================== 数据类型定义 ====================

/// 交易对行情
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Ticker {
    /// 交易对 ID，如 "BTC-USDT"
    #[serde(rename = "instId")]
    pub inst_id: String,
    /// 最新价格
    #[serde(rename = "last")]
    pub last_price: String,
    /// 24小时涨跌幅
    #[serde(rename = "open24h")]
    pub open_24h: String,
}

/// K 线数据
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Candle {
    /// 开盘时间戳 (ms)
    pub ts: i64,
    /// 开盘价
    pub open: f64,
    /// 最高价
    pub high: f64,
    /// 最低价
    pub low: f64,
    /// 收盘价
    pub close: f64,
    /// 成交量
    pub vol: f64,
}

impl Candle {
    /// 从 OKX API 响应解析
    /// OKX 返回格式: [ts, open, high, low, close, vol, volCcy]
    pub fn from_okx(data: &[String]) -> Result<Self> {
        if data.len() < 6 {
            return Err(anyhow::anyhow!("Invalid candle data length"));
        }
        Ok(Self {
            ts: data[0].parse()?,
            open: data[1].parse()?,
            high: data[2].parse()?,
            low: data[3].parse()?,
            close: data[4].parse()?,
            vol: data[5].parse()?,
        })
    }
}

/// 市场数据
#[derive(Debug, Clone)]
pub struct MarketData {
    /// 数据来源市场
    pub source: String,
    /// 交易对
    pub inst_id: String,
    /// 当前行情
    pub ticker: Option<Ticker>,
    /// K 线数据
    pub candles: Vec<Candle>,
    /// 计算出的涨跌幅 (%)
    pub price_change_pct: f64,
    /// 数据时间
    pub timestamp: DateTime<Utc>,
}

/// 交易信号类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SignalType {
    /// 买入信号
    Buy,
    /// 卖出信号
    Sell,
    /// 观望/提醒
    Alert,
}

/// 交易信号
#[derive(Debug, Clone, Serialize)]
pub struct Signal {
    /// 信号 ID
    pub id: String,
    /// 策略名称
    pub strategy_name: String,
    /// 信号类型
    pub signal_type: SignalType,
    /// 交易对
    pub inst_id: String,
    /// 信号描述
    pub description: String,
    /// 相关数据（JSON 格式）
    pub data: serde_json::Value,
    /// 生成时间
    pub created_at: DateTime<Utc>,
    /// 是否需要 AI 分析
    pub needs_analysis: bool,
}

/// 调度任务类型
#[derive(Debug, Clone)]
pub enum TaskType {
    /// 检查策略条件
    CheckConditions,
    /// 获取市场数据
    FetchMarketData,
    /// 自定义任务
    Custom(String),
}

/// 调度任务
pub struct ScheduledTask {
    /// 任务类型
    pub task_type: TaskType,
    /// 执行间隔
    pub interval: Duration,
    /// 任务函数
    pub handler: Box<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>> + Send + Sync>,
}

// ==================== 组件实现 ====================

/// 策略执行器
///
/// 负责管理策略列表，执行条件检查和信号生成
pub struct Executor {
    strategies: RwLock<Vec<Arc<dyn Strategy>>>,
}

impl Executor {
    pub fn new() -> Self {
        Self {
            strategies: RwLock::new(Vec::new()),
        }
    }

    /// 加载策略
    pub async fn load_strategy(&self, strategy: Arc<dyn Strategy>) {
        let mut strategies = self.strategies.write().await;
        strategies.push(strategy);
    }

    /// 执行所有策略的条件检查
    pub async fn execute(&self, data: &MarketData) -> Result<Vec<Signal>> {
        let strategies = self.strategies.read().await;
        let mut signals = Vec::new();

        for strategy in strategies.iter() {
            match strategy.check_conditions(data).await {
                Ok(true) => {
                    match strategy.generate_signal(data).await {
                        Ok(signal) => signals.push(signal),
                        Err(e) => {
                            eprintln!("Strategy {} failed to generate signal: {}", strategy.name(), e);
                        }
                    }
                }
                Ok(false) => {}
                Err(e) => {
                    eprintln!("Strategy {} check_conditions failed: {}", strategy.name(), e);
                }
            }
        }

        Ok(signals)
    }

    /// 获取已加载的策略数量
    pub async fn strategy_count(&self) -> usize {
        self.strategies.read().await.len()
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

/// 任务调度器
///
/// 负责定时任务的调度和执行
pub struct Scheduler {
    tasks: RwLock<Vec<(TaskType, Duration, mpsc::Sender<()>)>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            tasks: RwLock::new(Vec::new()),
        }
    }

    /// 调度周期性任务
    ///
    /// # Arguments
    /// * `task_type` - 任务类型
    /// * `interval` - 执行间隔
    /// * `handler` - 任务处理函数
    pub async fn schedule_repeating<F, Fut>(
        &self,
        task_type: TaskType,
        interval_duration: Duration,
        handler: F,
    ) -> Result<()>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        let (tx, mut rx) = mpsc::channel::<()>(1);
        let mut tasks = self.tasks.write().await;
        tasks.push((task_type, interval_duration, tx));
        drop(tasks);

        // 启动任务循环
        tokio::spawn(async move {
            let mut ticker = interval(interval_duration);

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        if let Err(e) = handler().await {
                            eprintln!("Scheduled task error: {}", e);
                        }
                    }
                    _ = rx.recv() => {
                        // 收到停止信号
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// 停止所有调度任务
    pub async fn stop_all(&self) {
        let tasks = self.tasks.read().await;
        for (_, _, tx) in tasks.iter() {
            let _ = tx.send(()).await;
        }
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// 市场监控器
///
/// 负责监控市场数据变化，检测异常
pub struct Monitor {
    markets: RwLock<Vec<Arc<dyn Market>>>,
    data_cache: RwLock<HashMap<String, MarketData>>,
}

impl Monitor {
    pub fn new() -> Self {
        Self {
            markets: RwLock::new(Vec::new()),
            data_cache: RwLock::new(HashMap::new()),
        }
    }

    /// 添加市场
    pub async fn add_market(&self, market: Arc<dyn Market>) {
        let mut markets = self.markets.write().await;
        markets.push(market);
    }

    /// 获取所有市场的行情数据
    pub async fn fetch_all_tickers(&self, inst_type: &str) -> Result<HashMap<String, Vec<Ticker>>> {
        let markets = self.markets.read().await;
        let mut result = HashMap::new();

        for market in markets.iter() {
            match market.fetch_tickers(inst_type).await {
                Ok(tickers) => {
                    result.insert(market.name().to_string(), tickers);
                }
                Err(e) => {
                    eprintln!("Market {} fetch_tickers failed: {}", market.name(), e);
                }
            }
        }

        Ok(result)
    }

    /// 获取指定交易对的 K 线数据并计算涨跌幅
    pub async fn fetch_candles_with_change(
        &self,
        market_name: &str,
        inst_id: &str,
        bar: &str,
        limit: u32,
    ) -> Result<Option<MarketData>> {
        let markets = self.markets.read().await;
        let market = markets
            .iter()
            .find(|m| m.name() == market_name)
            .ok_or_else(|| anyhow::anyhow!("Market {} not found", market_name))?;

        let candles = market.fetch_candles(inst_id, bar, limit).await?;

        if candles.is_empty() {
            return Ok(None);
        }

        // 计算涨跌幅
        let price_change_pct = Self::calc_price_change(&candles);

        let data = MarketData {
            source: market_name.to_string(),
            inst_id: inst_id.to_string(),
            ticker: None,
            candles,
            price_change_pct,
            timestamp: Utc::now(),
        };

        // 缓存数据
        let mut cache = self.data_cache.write().await;
        cache.insert(format!("{}:{}", market_name, inst_id), data.clone());

        Ok(Some(data))
    }

    /// 计算价格涨跌幅 (%)
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

    /// 检测超过阈值的变化
    pub async fn detect_threshold_crossing(
        &self,
        threshold_pct: f64,
    ) -> Result<Vec<MarketData>> {
        let cache = self.data_cache.read().await;
        let result: Vec<MarketData> = cache
            .values()
            .filter(|data| data.price_change_pct.abs() >= threshold_pct)
            .cloned()
            .collect();
        Ok(result)
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== Engine 核心 ====================

/// Engine 内部状态
struct EngineInner {
    /// 策略执行器
    executor: Executor,
    /// 任务调度器
    scheduler: Scheduler,
    /// 市场监控器
    monitor: Monitor,
    /// 通知渠道
    channels: RwLock<Vec<Arc<dyn Channel>>>,
    /// 当前状态
    state: RwLock<RuntimeState>,
}

/// Engine - 业务逻辑层核心
///
/// 负责策略执行、市场监控、信号生成和通知发送
pub struct Engine {
    inner: RwLock<EngineInner>,
}

impl Engine {
    /// 创建新的 Engine 实例
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(EngineInner {
                executor: Executor::new(),
                scheduler: Scheduler::new(),
                monitor: Monitor::new(),
                channels: RwLock::new(Vec::new()),
                state: RwLock::new(RuntimeState::Initialized),
            }),
        }
    }

    /// 加载策略
    pub async fn load_strategy(&self, strategy: Arc<dyn Strategy>) -> Result<()> {
        let inner = self.inner.read().await;
        inner.executor.load_strategy(strategy).await;
        Ok(())
    }

    /// 添加市场
    pub async fn add_market(&self, market: Arc<dyn Market>) -> Result<()> {
        let inner = self.inner.read().await;
        inner.monitor.add_market(market).await;
        Ok(())
    }

    /// 添加通知渠道
    pub async fn add_channel(&self, channel: Arc<dyn Channel>) -> Result<()> {
        let inner = self.inner.read().await;
        let mut channels = inner.channels.write().await;
        channels.push(channel);
        Ok(())
    }

    /// 调度周期性任务
    pub async fn schedule_repeating<F, Fut>(
        &self,
        task_type: TaskType,
        interval: Duration,
        handler: F,
    ) -> Result<()>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        let inner = self.inner.read().await;
        inner.scheduler.schedule_repeating(task_type, interval, handler).await
    }

    /// 检查所有策略条件
    pub async fn check_conditions(&self, data: &MarketData) -> Result<Vec<Signal>> {
        let inner = self.inner.read().await;
        inner.executor.execute(data).await
    }

    /// 发送通知到所有渠道
    pub async fn send_notification(&self, signal: &Signal) -> Result<()> {
        let inner = self.inner.read().await;
        let channels = inner.channels.read().await;
        let message = serde_json::to_string_pretty(signal)?;

        for channel in channels.iter() {
            if let Err(e) = channel.send(&message).await {
                eprintln!("Channel {} failed to send: {}", channel.name(), e);
            }
        }

        Ok(())
    }

    /// 获取市场监控器引用
    pub async fn monitor(&self) -> Result<tokio::sync::RwLockReadGuard<'_, Monitor>> {
        let inner = self.inner.read().await;
        // 由于 RwLockReadGuard 的生命周期问题，这里返回 Monitor 的引用方式需要调整
        // 实际使用时直接通过 Engine 方法暴露 Monitor 功能
        Ok(tokio::sync::RwLockReadGuard::map(inner, |i| &i.monitor))
    }

    /// 获取调度器引用
    pub fn scheduler(&self) -> &Scheduler {
        // 由于内部可变性设计，这里简化处理
        // 实际使用时通过 Engine 方法间接调用
        unimplemented!("Use schedule_repeating method instead")
    }

    /// 获取当前状态
    pub async fn state(&self) -> RuntimeState {
        let inner = self.inner.read().await;
        *inner.state.read().await
    }

    /// 异步触发 Agent 任务（不等待结果）
    ///
    /// 根据设计文档，Engine 通过 spawn 异步触发 Agent 任务，立即返回
    pub async fn spawn_agent_task<T>(&self, _task: T) {
        // TODO: 第四阶段实现 Agent 层时完善
        // 当前仅作为接口预留
        println!("Agent task spawned (to be implemented in Phase 4)");
    }
}

#[async_trait]
impl LifecycleHandler for Engine {
    async fn on_start(&self) -> Result<()> {
        let inner = self.inner.read().await;
        let mut state = inner.state.write().await;
        *state = RuntimeState::Running;
        println!("Engine started with {} strategies", inner.executor.strategy_count().await);
        Ok(())
    }

    async fn on_stop(&self) -> Result<()> {
        let inner = self.inner.read().await;
        inner.scheduler.stop_all().await;
        let mut state = inner.state.write().await;
        *state = RuntimeState::Stopped;
        println!("Engine stopped");
        Ok(())
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    struct TestStrategy;

    #[async_trait]
    impl Strategy for TestStrategy {
        fn name(&self) -> &str {
            "TestStrategy"
        }

        async fn check_conditions(&self, _data: &MarketData) -> Result<bool> {
            Ok(true)
        }

        async fn generate_signal(&self, data: &MarketData) -> Result<Signal> {
            Ok(Signal {
                id: "test-1".to_string(),
                strategy_name: self.name().to_string(),
                signal_type: SignalType::Buy,
                inst_id: data.inst_id.clone(),
                description: "Test signal".to_string(),
                data: serde_json::json!({}),
                created_at: Utc::now(),
                needs_analysis: false,
            })
        }
    }

    struct TestMarket;

    #[async_trait]
    impl Market for TestMarket {
        fn name(&self) -> &str {
            "TestMarket"
        }

        async fn fetch_tickers(&self, _inst_type: &str) -> Result<Vec<Ticker>> {
            Ok(vec![])
        }

        async fn fetch_candles(&self, _inst_id: &str, _bar: &str, _limit: u32) -> Result<Vec<Candle>> {
            Ok(vec![])
        }
    }

    struct TestChannel;

    #[async_trait]
    impl Channel for TestChannel {
        fn name(&self) -> &str {
            "TestChannel"
        }

        async fn send(&self, message: &str) -> Result<()> {
            println!("TestChannel received: {}", message);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_engine_lifecycle() {
        let engine = Arc::new(Engine::new());

        // 初始状态
        assert_eq!(engine.state().await, RuntimeState::Initialized);

        // 加载策略
        engine.load_strategy(Arc::new(TestStrategy)).await.unwrap();

        // 添加市场和渠道
        engine.add_market(Arc::new(TestMarket)).await.unwrap();
        engine.add_channel(Arc::new(TestChannel)).await.unwrap();

        // 启动
        engine.on_start().await.unwrap();
        assert_eq!(engine.state().await, RuntimeState::Running);

        // 停止
        engine.on_stop().await.unwrap();
        assert_eq!(engine.state().await, RuntimeState::Stopped);
    }

    #[tokio::test]
    async fn test_executor() {
        let executor = Executor::new();
        executor.load_strategy(Arc::new(TestStrategy)).await;
        assert_eq!(executor.strategy_count().await, 1);

        let data = MarketData {
            source: "test".to_string(),
            inst_id: "BTC-USDT".to_string(),
            ticker: None,
            candles: vec![],
            price_change_pct: 10.0,
            timestamp: Utc::now(),
        };

        let signals = executor.execute(&data).await.unwrap();
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].signal_type, SignalType::Buy);
    }

    #[tokio::test]
    async fn test_monitor_calc_change() {
        let candles = vec![
            Candle { ts: 1, open: 100.0, high: 110.0, low: 90.0, close: 105.0, vol: 1000.0 },
            Candle { ts: 2, open: 105.0, high: 115.0, low: 95.0, close: 110.0, vol: 1000.0 },
        ];

        let change = Monitor::calc_price_change(&candles);
        assert_eq!(change, 10.0); // (110 - 100) / 100 * 100 = 10%
    }
}
