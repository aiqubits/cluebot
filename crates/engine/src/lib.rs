use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use cluebot_runtime::{LifecycleHandler, RuntimeState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration};

// ==================== Agent Task Types (for Agent layer) ====================

/// Agent task types
/// 
/// Defines task types that Engine can asynchronously trigger to the Agent layer
#[derive(Debug, Clone)]
pub enum AgentTask {
    /// Strategy discovery - analyze market data to find trading opportunities
    DiscoverStrategy(MarketData),
    /// Signal analysis - analyze specific trading signals
    AnalyzeSignal(Signal),
    /// Pattern recognition - identify historical patterns
    RecognizePattern(Vec<MarketData>),
}

/// Agent task handler trait
/// 
/// Implemented by Agent layer, Engine triggers tasks asynchronously through this trait
#[async_trait]
pub trait AgentTaskHandler: Send + Sync {
    /// Process task asynchronously (no return result)
    async fn handle_task(&self, task: AgentTask);
}

// ==================== Trait Definitions ====================

/// Strategy trait - defines common interface for strategies
///
/// All concrete strategies need to implement this trait to be called by Engine
/// 
/// Option B: Strategy fetches market data autonomously
#[async_trait]
pub trait Strategy: Send + Sync {
    /// Get strategy name
    fn name(&self) -> &str;

    /// Execute strategy
    ///
    /// Strategy fetches market data autonomously, analyzes and returns signal list
    ///
    /// # Arguments
    /// * `market` - Market interface, strategy can fetch data through it
    ///
    /// # Returns
    /// * `Ok(Vec<Signal>)` - Trading signal list (may be empty)
    async fn execute(&self, market: &dyn Market) -> Result<Vec<Signal>>;
}

/// Market trait - defines market data fetch interface
///
/// Exchanges like OKX, Binance implement this trait
#[async_trait]
pub trait Market: Send + Sync {
    /// Get market name
    fn name(&self) -> &str;

    /// Fetch all trading pair tickers
    async fn fetch_tickers(&self, inst_type: &str) -> Result<Vec<Ticker>>;

    /// Fetch candlestick data
    ///
    /// # Arguments
    /// * `inst_id` - Trading pair ID, e.g. "BTC-USDT"
    /// * `bar` - Time period, e.g. "1H"
    /// * `limit` - Number of records to return
    async fn fetch_candles(&self, inst_id: &str, bar: &str, limit: u32) -> Result<Vec<Candle>>;
}

/// Notification channel trait - defines notification send interface
///
/// Channels like Lark, Email implement this trait
#[async_trait]
pub trait Channel: Send + Sync {
    /// Get channel name
    fn name(&self) -> &str;

    /// Send notification
    ///
    /// # Arguments
    /// * `message` - Notification content
    async fn send(&self, message: &str) -> Result<()>;
}

// ==================== Data Type Definitions ====================

/// Trading pair ticker
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Ticker {
    /// Trading pair ID, e.g. "BTC-USDT"
    #[serde(rename = "instId")]
    pub inst_id: String,
    /// Latest price
    #[serde(rename = "last")]
    pub last_price: String,
    /// 24-hour price change
    #[serde(rename = "open24h")]
    pub open_24h: String,
}

/// Candlestick data
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Candle {
    /// Opening timestamp (ms)
    pub ts: i64,
    /// Opening price
    pub open: f64,
    /// Highest price
    pub high: f64,
    /// Lowest price
    pub low: f64,
    /// Closing price
    pub close: f64,
    /// Trading volume
    pub vol: f64,
}

impl Candle {
    /// Parse from OKX API response
    /// OKX returns format: [ts, open, high, low, close, vol, volCcy]
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

/// Market data
#[derive(Debug, Clone)]
pub struct MarketData {
    /// Data source market
    pub source: String,
    /// Trading pair
    pub inst_id: String,
    /// Current ticker
    pub ticker: Option<Ticker>,
    /// Candlestick data
    pub candles: Vec<Candle>,
    /// Calculated price change (%)
    pub price_change_pct: f64,
    /// Data timestamp
    pub timestamp: DateTime<Utc>,
}

/// Trading signal type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SignalType {
    /// Buy signal
    Buy,
    /// Sell signal
    Sell,
    /// Alert/Watch
    Alert,
}

/// Trading signal
#[derive(Debug, Clone, Serialize)]
pub struct Signal {
    /// Signal ID
    pub id: String,
    /// Strategy name
    pub strategy_name: String,
    /// Signal type
    pub signal_type: SignalType,
    /// Trading pair
    pub inst_id: String,
    /// Signal description
    pub description: String,
    /// Related data (JSON format)
    pub data: serde_json::Value,
    /// Creation time
    pub created_at: DateTime<Utc>,
    /// Whether AI analysis is needed
    pub needs_analysis: bool,
}

/// Scheduled task type
#[derive(Debug, Clone)]
pub enum TaskType {
    /// Check strategy conditions
    CheckConditions,
    /// Fetch market data
    FetchMarketData,
    /// Custom task
    Custom(String),
}

/// Scheduled task
pub struct ScheduledTask {
    /// Task type
    pub task_type: TaskType,
    /// Execution interval
    pub interval: Duration,
    /// Task handler function
    pub handler: Box<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>> + Send + Sync>,
}

// ==================== Component Implementations ====================

/// Strategy executor
///
/// Manages strategy list, executes strategies to fetch data and generate signals
/// 
/// Option B: Strategy fetches market data autonomously
pub struct Executor {
    strategies: RwLock<Vec<Arc<dyn Strategy>>>,
}

impl Executor {
    pub fn new() -> Self {
        Self {
            strategies: RwLock::new(Vec::new()),
        }
    }

    /// Load strategy
    pub async fn load_strategy(&self, strategy: Arc<dyn Strategy>) {
        let mut strategies = self.strategies.write().await;
        strategies.push(strategy);
    }

    /// Execute all strategies
    ///
    /// Each strategy fetches market data autonomously and returns signals
    pub async fn execute(&self, market: &dyn Market) -> Result<Vec<Signal>> {
        let strategies = self.strategies.read().await;
        let mut all_signals = Vec::new();

        for strategy in strategies.iter() {
            match strategy.execute(market).await {
                Ok(signals) => {
                    all_signals.extend(signals);
                }
                Err(e) => {
                    eprintln!("Strategy {} execute failed: {}", strategy.name(), e);
                }
            }
        }

        Ok(all_signals)
    }

    /// Get number of loaded strategies
    pub async fn strategy_count(&self) -> usize {
        self.strategies.read().await.len()
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

/// Task scheduler
///
/// Responsible for scheduling and executing timed tasks
pub struct Scheduler {
    tasks: RwLock<Vec<(TaskType, Duration, mpsc::Sender<()>)>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            tasks: RwLock::new(Vec::new()),
        }
    }

    /// Schedule periodic task
    ///
    /// # Arguments
    /// * `task_type` - Task type
    /// * `interval` - Execution interval
    /// * `handler` - Task handler function
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

        // Start task loop
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
                        // Received stop signal
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop all scheduled tasks
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

/// Market monitor
///
/// Monitors market data changes and detects anomalies
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

    /// Add market
    pub async fn add_market(&self, market: Arc<dyn Market>) {
        let mut markets = self.markets.write().await;
        markets.push(market);
    }

    /// Fetch all market tickers
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

    /// Fetch candlestick data for specified trading pair and calculate price change
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

        // Calculate price change
        let price_change_pct = Self::calc_price_change(&candles);

        let data = MarketData {
            source: market_name.to_string(),
            inst_id: inst_id.to_string(),
            ticker: None,
            candles,
            price_change_pct,
            timestamp: Utc::now(),
        };

        // Cache data
        let mut cache = self.data_cache.write().await;
        cache.insert(format!("{}:{}", market_name, inst_id), data.clone());

        Ok(Some(data))
    }

    /// Calculate price change (%)
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

    /// Detect changes exceeding threshold
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

// ==================== Engine Core ====================

/// Engine internal state
struct EngineInner {
    /// Strategy executor
    executor: Executor,
    /// Task scheduler
    scheduler: Scheduler,
    /// Market monitor
    monitor: Monitor,
    /// Notification channels
    channels: RwLock<Vec<Arc<dyn Channel>>>,
    /// Agent task handler
    agent_handler: RwLock<Option<Arc<dyn AgentTaskHandler>>>,
    /// Current state
    state: RwLock<RuntimeState>,
}

/// Engine - Business logic layer core
///
/// Responsible for strategy execution, market monitoring, signal generation and notification sending
pub struct Engine {
    inner: RwLock<EngineInner>,
}

impl Engine {
    /// Create new Engine instance
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(EngineInner {
                executor: Executor::new(),
                scheduler: Scheduler::new(),
                monitor: Monitor::new(),
                channels: RwLock::new(Vec::new()),
                agent_handler: RwLock::new(None),
                state: RwLock::new(RuntimeState::Initialized),
            }),
        }
    }

    /// Load strategy
    pub async fn load_strategy(&self, strategy: Arc<dyn Strategy>) -> Result<()> {
        let inner = self.inner.read().await;
        inner.executor.load_strategy(strategy).await;
        Ok(())
    }

    /// Add market
    pub async fn add_market(&self, market: Arc<dyn Market>) -> Result<()> {
        let inner = self.inner.read().await;
        inner.monitor.add_market(market).await;
        Ok(())
    }

    /// Add notification channel
    pub async fn add_channel(&self, channel: Arc<dyn Channel>) -> Result<()> {
        let inner = self.inner.read().await;
        let mut channels = inner.channels.write().await;
        channels.push(channel);
        Ok(())
    }

    /// Schedule periodic task
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

    /// Execute all strategies
    ///
    /// Strategy fetches market data autonomously and generates signals
    pub async fn execute_strategies(&self, market: &dyn Market) -> Result<Vec<Signal>> {
        let inner = self.inner.read().await;
        inner.executor.execute(market).await
    }

    /// Send notification to all channels
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

    /// Get market monitor reference
    pub async fn monitor(&self) -> Result<tokio::sync::RwLockReadGuard<'_, Monitor>> {
        let inner = self.inner.read().await;
        // Due to RwLockReadGuard lifetime issues, the way to return Monitor reference needs adjustment
        // In actual use, expose Monitor functionality directly through Engine methods
        Ok(tokio::sync::RwLockReadGuard::map(inner, |i| &i.monitor))
    }

    /// Get scheduler reference
    pub fn scheduler(&self) -> &Scheduler {
        // Simplified due to interior mutability design
        // In actual use, call indirectly through Engine methods
        unimplemented!("Use schedule_repeating method instead")
    }

    /// Get current state
    pub async fn state(&self) -> RuntimeState {
        let inner = self.inner.read().await;
        *inner.state.read().await
    }

    /// Set Agent task handler
    ///
    /// # Arguments
    /// * `handler` - Agent task handler
    pub async fn set_agent_handler(&self, handler: Arc<dyn AgentTaskHandler>) {
        let inner = self.inner.read().await;
        let mut agent_handler = inner.agent_handler.write().await;
        *agent_handler = Some(handler);
    }

    /// Asynchronously trigger Agent task (no wait for result)
    ///
    /// According to design doc, Engine triggers Agent tasks asynchronously via spawn, returns immediately
    ///
    /// # Arguments
    /// * `task` - Agent task
    pub async fn spawn_agent_task(&self, task: AgentTask) {
        let inner = self.inner.read().await;
        let handler = inner.agent_handler.read().await.clone();
        
        match handler {
            Some(h) => {
                // Async spawn, no wait for result
                tokio::spawn(async move {
                    h.handle_task(task).await;
                });
            }
            None => {
                eprintln!("Agent handler not set, task {:?} dropped", task);
            }
        }
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

// ==================== Tests ====================

#[cfg(test)]
mod tests {
    use super::*;

    struct TestStrategy;

    #[async_trait]
    impl Strategy for TestStrategy {
        fn name(&self) -> &str {
            "TestStrategy"
        }

        async fn execute(&self, market: &dyn Market) -> Result<Vec<Signal>> {
            // Test strategy: fetch data autonomously and generate signals
            let candles = market.fetch_candles("TEST-USDT", "1H", 8).await?;
            
            if candles.is_empty() {
                return Ok(vec![]);
            }
            
            let signal = Signal {
                id: "test-1".to_string(),
                strategy_name: self.name().to_string(),
                signal_type: SignalType::Buy,
                inst_id: "TEST-USDT".to_string(),
                description: "Test signal".to_string(),
                data: serde_json::json!({"candles_count": candles.len()}),
                created_at: Utc::now(),
                needs_analysis: false,
            };
            
            Ok(vec![signal])
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

        // Initial state
        assert_eq!(engine.state().await, RuntimeState::Initialized);

        // Load strategy
        engine.load_strategy(Arc::new(TestStrategy)).await.unwrap();

        // Add market and channel
        engine.add_market(Arc::new(TestMarket)).await.unwrap();
        engine.add_channel(Arc::new(TestChannel)).await.unwrap();

        // Start
        engine.on_start().await.unwrap();
        assert_eq!(engine.state().await, RuntimeState::Running);

        // Stop
        engine.on_stop().await.unwrap();
        assert_eq!(engine.state().await, RuntimeState::Stopped);
    }

    #[tokio::test]
    async fn test_executor() {
        let executor = Executor::new();
        executor.load_strategy(Arc::new(TestStrategy)).await;
        assert_eq!(executor.strategy_count().await, 1);

        let market = TestMarket;
        let signals = executor.execute(&market).await.unwrap();
        // TestMarket returns empty candles, so strategy returns empty signal list
        assert_eq!(signals.len(), 0);
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
