use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use cluebot_engine::{Channel, MarketData, Signal};
use cluebot_llm_gateway::{LLMGateway, Message};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

// ==================== Core Type Definitions ====================

/// Agent task types
#[derive(Debug, Clone)]
pub enum AgentTask {
    /// Strategy discovery - analyze market data to find trading opportunities
    DiscoverStrategy(MarketData),
    /// Signal analysis - analyze specific trading signals
    AnalyzeSignal(Signal),
    /// Pattern recognition - identify historical patterns
    RecognizePattern(Vec<MarketData>),
}

/// Task type enum (for reports)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AgentTaskType {
    DiscoverStrategy,
    AnalyzeSignal,
    RecognizePattern,
}

/// Discovered trading opportunity
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    /// Opportunity type
    pub opportunity_type: String,
    /// Confidence level (0.0 - 1.0)
    pub confidence: f64,
    /// Description
    pub description: String,
    /// Supporting data
    pub supporting_data: serde_json::Value,
}

/// Recommended action
#[derive(Debug, Clone, Serialize)]
pub struct Recommendation {
    /// Action type
    pub action_type: String,
    /// Priority (high/medium/low)
    pub priority: String,
    /// Description
    pub description: String,
    /// Expected outcome
    pub expected_outcome: String,
}

/// Analysis result report
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisReport {
    /// Report ID
    pub report_id: String,
    /// Task type
    pub task_type: AgentTaskType,
    /// Summary
    pub summary: String,
    /// Discovered opportunities
    pub findings: Vec<Finding>,
    /// Recommendations
    pub recommendations: Vec<Recommendation>,
    /// Risk level
    pub risk_level: String,
    /// Generation time
    pub timestamp: DateTime<Utc>,
}

impl AnalysisReport {
    /// Create new report
    pub fn new(task_type: AgentTaskType) -> Self {
        Self {
            report_id: Uuid::new_v4().to_string(),
            task_type,
            summary: String::new(),
            findings: Vec::new(),
            recommendations: Vec::new(),
            risk_level: "low".to_string(),
            timestamp: Utc::now(),
        }
    }

    /// Add finding
    pub fn add_finding(mut self, finding: Finding) -> Self {
        self.findings.push(finding);
        self
    }

    /// Add recommendation
    pub fn add_recommendation(mut self, rec: Recommendation) -> Self {
        self.recommendations.push(rec);
        self
    }

    /// Set summary
    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = summary.into();
        self
    }

    /// Set risk level
    pub fn with_risk_level(mut self, level: impl Into<String>) -> Self {
        self.risk_level = level.into();
        self
    }
}

// ==================== Agent Trait ====================

/// Agent trait - all Agents implement this interface
#[async_trait]
pub trait Agent: Send + Sync {
    /// Get Agent name
    fn name(&self) -> &str;

    /// Execute analysis task
    ///
    /// # Arguments
    /// * `task` - Agent task
    ///
    /// # Returns
    /// * `Ok(AnalysisReport)` - Analysis result report
    async fn execute(&self, task: AgentTask) -> Result<AnalysisReport>;

    /// Generate and send report
    ///
    /// # Arguments
    /// * `report` - Analysis report
    async fn send_report(&self, report: &AnalysisReport) -> Result<()>;
}

// ==================== Market Data Analyzer ====================

/// Market data analyzer
///
/// Responsible for analyzing raw market data and extracting key indicators
pub struct Analyzer;

impl Analyzer {
    /// Analyze market data
    ///
    /// # Arguments
    /// * `data` - Market data
    ///
    /// # Returns
    /// * Analysis result summary
    pub fn analyze_market_data(data: &MarketData) -> String {
        let mut analysis = String::new();

        analysis.push_str(&format!("Trading Pair: {}\n", data.inst_id));
        analysis.push_str(&format!("Data Source: {}\n", data.source));
        analysis.push_str(&format!("Price Change: {:.2}%\n", data.price_change_pct));
        analysis.push_str(&format!("Candle Count: {}\n", data.candles.len()));

        if !data.candles.is_empty() {
            let first = &data.candles[0];
            let last = &data.candles[data.candles.len() - 1];
            analysis.push_str(&format!("Open Price: {:.2}\n", first.open));
            analysis.push_str(&format!("Latest Price: {:.2}\n", last.close));
            analysis.push_str(&format!("Highest Price: {:.2}\n", 
                data.candles.iter().map(|c| c.high).fold(0.0f64, f64::max)));
            analysis.push_str(&format!("Lowest Price: {:.2}\n",
                data.candles.iter().map(|c| c.low).fold(f64::MAX, f64::min)));
        }

        analysis
    }

    /// Analyze multiple market data
    pub fn analyze_multiple_data(data_list: &[MarketData]) -> String {
        let mut analysis = String::new();
        analysis.push_str(&format!("Analyzing {} trading pairs\n\n", data_list.len()));

        for data in data_list.iter().take(5) {
            analysis.push_str(&format!("- {}: {:.2}%\n", data.inst_id, data.price_change_pct));
        }

        if data_list.len() > 5 {
            analysis.push_str(&format!("... and {} more trading pairs\n", data_list.len() - 5));
        }

        analysis
    }
}

// ==================== Pattern Recognizer ====================

/// Pattern recognizer
///
/// Identifies patterns and trends in market data
pub struct PatternRecognizer;

impl PatternRecognizer {
    /// Recognize trend pattern
    pub fn recognize_trend(data: &MarketData) -> Option<String> {
        if data.candles.len() < 2 {
            return None;
        }

        let change = data.price_change_pct;

        if change > 10.0 {
            Some(format!("Strong upward trend (+{:.1}%)", change))
        } else if change > 5.0 {
            Some(format!("Moderate upward trend (+{:.1}%)", change))
        } else if change < -10.0 {
            Some(format!("Strong downward trend ({:.1}%)", change))
        } else if change < -5.0 {
            Some(format!("Moderate downward trend ({:.1}%)", change))
        } else {
            Some(format!("Sideways movement ({:.1}%)", change))
        }
    }

    /// Recognize volatility
    pub fn recognize_volatility(data: &MarketData) -> Option<String> {
        if data.candles.len() < 2 {
            return None;
        }

        let volatility: f64 = data.candles.iter()
            .map(|c| ((c.high - c.low) / c.open).abs())
            .sum::<f64>() / data.candles.len() as f64 * 100.0;

        if volatility > 5.0 {
            Some(format!("High volatility ({:.1}%)", volatility))
        } else if volatility > 2.0 {
            Some(format!("Medium volatility ({:.1}%)", volatility))
        } else {
            Some(format!("Low volatility ({:.1}%)", volatility))
        }
    }

    /// Recognize breakout pattern
    pub fn recognize_breakout(data: &MarketData) -> Option<String> {
        if data.candles.len() < 5 {
            return None;
        }

        let recent_high = data.candles.iter().rev().take(3)
            .map(|c| c.high).fold(0.0f64, f64::max);
        let previous_high = data.candles.iter().rev().skip(3).take(5)
            .map(|c| c.high).fold(0.0f64, f64::max);

        if recent_high > previous_high * 1.02 {
            Some("Upward breakout".to_string())
        } else if recent_high < previous_high * 0.98 {
            Some("Downward breakout".to_string())
        } else {
            None
        }
    }
}

// ==================== Discovery Agent ====================

/// Discovery Agent
///
/// Responsible for discovering trading opportunities and generating analysis reports
pub struct DiscoveryAgent {
    /// LLM Gateway
    llm_gateway: Arc<LLMGateway>,
    /// Notification channels
    channels: RwLock<Vec<Arc<dyn Channel>>>,
    /// Agent name
    name: String,
}

impl DiscoveryAgent {
    /// Create new Discovery Agent
    pub fn new(llm_gateway: Arc<LLMGateway>) -> Self {
        Self {
            llm_gateway,
            channels: RwLock::new(Vec::new()),
            name: "DiscoveryAgent".to_string(),
        }
    }

    /// Add notification channel
    pub async fn add_channel(&self, channel: Arc<dyn Channel>) {
        let mut channels = self.channels.write().await;
        channels.push(channel);
    }

    /// Execute strategy discovery
    async fn discover_strategy(&self, data: &MarketData) -> Result<AnalysisReport> {
        // 1. Analyze market data
        let analysis = Analyzer::analyze_market_data(data);

        // 2. Recognize patterns
        let trend = PatternRecognizer::recognize_trend(data);
        let volatility = PatternRecognizer::recognize_volatility(data);
        let breakout = PatternRecognizer::recognize_breakout(data);

        // 3. Build prompt
        let prompt = self.build_discovery_prompt(&analysis, &trend, &volatility, &breakout);

        // 4. Call LLM for analysis
        let messages = vec![
            Message::system("You are a professional quantitative trading strategy analyst."),
            Message::user(prompt),
        ];

        let response = self.llm_gateway.chat(&messages).await?;

        // 5. Parse result and generate report
        let report = self.parse_discovery_response(&response.content, data);

        Ok(report)
    }

    /// Execute signal analysis
    async fn analyze_signal(&self, signal: &Signal) -> Result<AnalysisReport> {
        let prompt = format!(
            "Please analyze the following trading signal:\n\n\
            Signal ID: {}\n\
            Strategy: {}\n\
            Type: {:?}\n\
            Trading Pair: {}\n\
            Description: {}\n\
            Data: {}\n\n\
            Please provide:\n\
            1. Signal quality assessment\n\
            2. Potential risk points\n\
            3. Execution recommendations",
            signal.id,
            signal.strategy_name,
            signal.signal_type,
            signal.inst_id,
            signal.description,
            signal.data
        );

        let messages = vec![
            Message::system("You are a professional trading signal analyst."),
            Message::user(prompt),
        ];

        let response = self.llm_gateway.chat(&messages).await?;

        let report = AnalysisReport::new(AgentTaskType::AnalyzeSignal)
            .with_summary(format!("Signal analysis completed: {}", signal.inst_id))
            .add_finding(Finding {
                opportunity_type: "SignalAnalysis".to_string(),
                confidence: 0.8,
                description: response.content.clone(),
                supporting_data: serde_json::json!({
                    "signal_id": signal.id,
                    "analysis": response.content
                }),
            })
            .add_recommendation(Recommendation {
                action_type: "Review".to_string(),
                priority: "medium".to_string(),
                description: "Please carefully review the signal analysis results".to_string(),
                expected_outcome: "Make informed trading decisions".to_string(),
            });

        Ok(report)
    }

    /// Execute pattern recognition
    async fn recognize_pattern(&self, data_list: &[MarketData]) -> Result<AnalysisReport> {
        let analysis = Analyzer::analyze_multiple_data(data_list);

        let prompt = format!(
            "Please analyze the following multiple trading pairs' market data and identify common patterns:\n\n{}\n\n\
            Please identify:\n\
            1. Overall market trend\n\
            2. Sector rotation patterns\n\
            3. Correlation analysis\n\
            4. Potential arbitrage opportunities",
            analysis
        );

        let messages = vec![
            Message::system("You are a professional market pattern recognition expert."),
            Message::user(prompt),
        ];

        let response = self.llm_gateway.chat(&messages).await?;

        let report = AnalysisReport::new(AgentTaskType::RecognizePattern)
            .with_summary(format!("Pattern recognition completed: analyzed {} trading pairs", data_list.len()))
            .add_finding(Finding {
                opportunity_type: "PatternRecognition".to_string(),
                confidence: 0.75,
                description: response.content,
                supporting_data: serde_json::json!({
                    "data_count": data_list.len()
                }),
            });

        Ok(report)
    }

    /// Build strategy discovery prompt
    fn build_discovery_prompt(
        &self,
        analysis: &str,
        trend: &Option<String>,
        volatility: &Option<String>,
        breakout: &Option<String>,
    ) -> String {
        let mut prompt = format!(
            "Please analyze the following market data and identify potential trading opportunities:\n\n{}\n",
            analysis
        );

        if let Some(t) = trend {
            prompt.push_str(&format!("\nTrend Recognition: {}\n", t));
        }
        if let Some(v) = volatility {
            prompt.push_str(&format!("Volatility: {}\n", v));
        }
        if let Some(b) = breakout {
            prompt.push_str(&format!("Breakout Pattern: {}\n", b));
        }

        prompt.push_str("\nPlease provide:\n");
        prompt.push_str("1. Trading opportunity assessment\n");
        prompt.push_str("2. Recommended strategy type\n");
        prompt.push_str("3. Risk level assessment\n");
        prompt.push_str("4. Specific operation recommendations\n");

        prompt
    }

    /// Parse LLM response
    fn parse_discovery_response(&self, response: &str, data: &MarketData) -> AnalysisReport {
        // Simplified implementation: directly build report
        // In production, can use JSON to parse response
        AnalysisReport::new(AgentTaskType::DiscoverStrategy)
            .with_summary(format!("Strategy discovery: {}", data.inst_id))
            .add_finding(Finding {
                opportunity_type: "MarketAnalysis".to_string(),
                confidence: 0.7,
                description: response.to_string(),
                supporting_data: serde_json::json!({
                    "inst_id": data.inst_id,
                    "change_pct": data.price_change_pct
                }),
            })
            .add_recommendation(Recommendation {
                action_type: "Monitor".to_string(),
                priority: if data.price_change_pct.abs() > 10.0 {
                    "high".to_string()
                } else {
                    "medium".to_string()
                },
                description: "Continue monitoring this trading pair".to_string(),
                expected_outcome: "Capture trading opportunities".to_string(),
            })
    }
}

#[async_trait]
impl Agent for DiscoveryAgent {
    fn name(&self) -> &str {
        &self.name
    }

    async fn execute(&self, task: AgentTask) -> Result<AnalysisReport> {
        match task {
            AgentTask::DiscoverStrategy(data) => self.discover_strategy(&data).await,
            AgentTask::AnalyzeSignal(signal) => self.analyze_signal(&signal).await,
            AgentTask::RecognizePattern(data_list) => self.recognize_pattern(&data_list).await,
        }
    }

    async fn send_report(&self, report: &AnalysisReport) -> Result<()> {
        let channels = self.channels.read().await;
        let message = serde_json::to_string_pretty(report)?;

        for channel in channels.iter() {
            if let Err(e) = channel.send(&message).await {
                eprintln!("Channel {} failed to send report: {}", channel.name(), e);
            }
        }

        Ok(())
    }
}

// ==================== Agent Manager ====================

/// Agent manager
///
/// Manages all Agent instances, responsible for task distribution
pub struct AgentManager {
    agents: RwLock<HashMap<String, Arc<dyn Agent>>>,
}

impl AgentManager {
    /// Create new Agent manager
    pub fn new() -> Self {
        Self {
            agents: RwLock::new(HashMap::new()),
        }
    }

    /// Register Agent
    pub async fn register_agent(&self, name: String, agent: Arc<dyn Agent>) {
        let mut agents = self.agents.write().await;
        agents.insert(name, agent);
    }

    /// Get Agent
    pub async fn get_agent(&self, name: &str) -> Option<Arc<dyn Agent>> {
        let agents = self.agents.read().await;
        agents.get(name).cloned()
    }

    /// Execute task asynchronously (no wait for result)
    pub async fn spawn_task(&self, agent_name: &str, task: AgentTask) {
        let agent = match self.get_agent(agent_name).await {
            Some(a) => a,
            None => {
                eprintln!("Agent '{}' not found", agent_name);
                return;
            }
        };

        // Async spawn, no wait for result
        tokio::spawn(async move {
            match agent.execute(task).await {
                Ok(report) => {
                    println!("Agent task completed: {}", report.report_id);
                    if let Err(e) = agent.send_report(&report).await {
                        eprintln!("Failed to send report: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("Agent task failed: {}", e);
                }
            }
        });
    }
}

impl Default for AgentManager {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
    use super::*;
    use cluebot_engine::Candle;

    fn create_test_market_data() -> MarketData {
        MarketData {
            source: "test".to_string(),
            inst_id: "BTC-USDT".to_string(),
            ticker: None,
            candles: vec![
                Candle { ts: 1, open: 100.0, high: 110.0, low: 90.0, close: 105.0, vol: 1000.0 },
                Candle { ts: 2, open: 105.0, high: 115.0, low: 95.0, close: 110.0, vol: 1000.0 },
            ],
            price_change_pct: 10.0,
            timestamp: Utc::now(),
        }
    }

    // fn create_test_signal() -> Signal {
    //     Signal {
    //         id: "test-signal-1".to_string(),
    //         strategy_name: "TestStrategy".to_string(),
    //         signal_type: SignalType::Buy,
    //         inst_id: "BTC-USDT".to_string(),
    //         description: "Test buy signal".to_string(),
    //         data: serde_json::json!({"price": 100.0}),
    //         created_at: Utc::now(),
    //         needs_analysis: true,
    //     }
    // }

    #[test]
    fn test_analyzer() {
        let data = create_test_market_data();
        let analysis = Analyzer::analyze_market_data(&data);
        assert!(analysis.contains("BTC-USDT"));
        assert!(analysis.contains("10.00%"));
    }

    #[test]
    fn test_pattern_recognizer() {
        let data = create_test_market_data();
        let trend = PatternRecognizer::recognize_trend(&data);
        assert!(trend.is_some());
        assert!(trend.unwrap().contains("upward"));
    }

    #[test]
    fn test_analysis_report_builder() {
        let report = AnalysisReport::new(AgentTaskType::DiscoverStrategy)
            .with_summary("Test summary")
            .with_risk_level("medium")
            .add_finding(Finding {
                opportunity_type: "Test".to_string(),
                confidence: 0.8,
                description: "Test finding".to_string(),
                supporting_data: serde_json::json!({}),
            });

        assert_eq!(report.task_type, AgentTaskType::DiscoverStrategy);
        assert_eq!(report.summary, "Test summary");
        assert_eq!(report.risk_level, "medium");
        assert_eq!(report.findings.len(), 1);
    }

    #[tokio::test]
    async fn test_agent_manager() {
        let manager = AgentManager::new();
        
        // Create mock agent
        struct MockAgent;
        
        #[async_trait]
        impl Agent for MockAgent {
            fn name(&self) -> &str {
                "MockAgent"
            }

            async fn execute(&self, _task: AgentTask) -> Result<AnalysisReport> {
                Ok(AnalysisReport::new(AgentTaskType::DiscoverStrategy))
            }

            async fn send_report(&self, _report: &AnalysisReport) -> Result<()> {
                Ok(())
            }
        }

        manager.register_agent("mock".to_string(), Arc::new(MockAgent)).await;
        
        let agent = manager.get_agent("mock").await;
        assert!(agent.is_some());
        assert_eq!(agent.unwrap().name(), "MockAgent");
    }
}
