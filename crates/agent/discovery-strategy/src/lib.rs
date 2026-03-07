use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use cluebot_engine::{Channel, MarketData, Signal};
use cluebot_llm_gateway::{LLMGateway, Message};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

// ==================== 核心类型定义 ====================

/// Agent 任务类型
#[derive(Debug, Clone)]
pub enum AgentTask {
    /// 策略发现 - 分析市场数据发现交易机会
    DiscoverStrategy(MarketData),
    /// 信号分析 - 分析特定交易信号
    AnalyzeSignal(Signal),
    /// 模式识别 - 识别历史模式
    RecognizePattern(Vec<MarketData>),
}

/// 任务类型枚举（用于报告）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AgentTaskType {
    DiscoverStrategy,
    AnalyzeSignal,
    RecognizePattern,
}

/// 发现的交易机会
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    /// 机会类型
    pub opportunity_type: String,
    /// 置信度 (0.0 - 1.0)
    pub confidence: f64,
    /// 描述
    pub description: String,
    /// 支持数据
    pub supporting_data: serde_json::Value,
}

/// 建议操作
#[derive(Debug, Clone, Serialize)]
pub struct Recommendation {
    /// 建议类型
    pub action_type: String,
    /// 优先级 (high/medium/low)
    pub priority: String,
    /// 建议描述
    pub description: String,
    /// 预期效果
    pub expected_outcome: String,
}

/// 分析结果报告
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisReport {
    /// 报告 ID
    pub report_id: String,
    /// 任务类型
    pub task_type: AgentTaskType,
    /// 摘要
    pub summary: String,
    /// 发现的机会
    pub findings: Vec<Finding>,
    /// 建议
    pub recommendations: Vec<Recommendation>,
    /// 风险等级
    pub risk_level: String,
    /// 生成时间
    pub timestamp: DateTime<Utc>,
}

impl AnalysisReport {
    /// 创建新报告
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

    /// 添加发现
    pub fn add_finding(mut self, finding: Finding) -> Self {
        self.findings.push(finding);
        self
    }

    /// 添加建议
    pub fn add_recommendation(mut self, rec: Recommendation) -> Self {
        self.recommendations.push(rec);
        self
    }

    /// 设置摘要
    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = summary.into();
        self
    }

    /// 设置风险等级
    pub fn with_risk_level(mut self, level: impl Into<String>) -> Self {
        self.risk_level = level.into();
        self
    }
}

// ==================== Agent Trait ====================

/// Agent trait - 所有 Agent 实现此接口
#[async_trait]
pub trait Agent: Send + Sync {
    /// 获取 Agent 名称
    fn name(&self) -> &str;

    /// 执行分析任务
    ///
    /// # Arguments
    /// * `task` - Agent 任务
    ///
    /// # Returns
    /// * `Ok(AnalysisReport)` - 分析结果报告
    async fn execute(&self, task: AgentTask) -> Result<AnalysisReport>;

    /// 生成并发送报告
    ///
    /// # Arguments
    /// * `report` - 分析报告
    async fn send_report(&self, report: &AnalysisReport) -> Result<()>;
}

// ==================== 市场数据分析器 ====================

/// 市场数据分析器
///
/// 负责分析原始市场数据，提取关键指标
pub struct Analyzer;

impl Analyzer {
    /// 分析市场数据
    ///
    /// # Arguments
    /// * `data` - 市场数据
    ///
    /// # Returns
    /// * 分析结果摘要
    pub fn analyze_market_data(data: &MarketData) -> String {
        let mut analysis = String::new();

        analysis.push_str(&format!("交易对: {}\n", data.inst_id));
        analysis.push_str(&format!("数据来源: {}\n", data.source));
        analysis.push_str(&format!("涨跌幅: {:.2}%\n", data.price_change_pct));
        analysis.push_str(&format!("K线数量: {}\n", data.candles.len()));

        if !data.candles.is_empty() {
            let first = &data.candles[0];
            let last = &data.candles[data.candles.len() - 1];
            analysis.push_str(&format!("开盘价: {:.2}\n", first.open));
            analysis.push_str(&format!("最新价: {:.2}\n", last.close));
            analysis.push_str(&format!("最高价: {:.2}\n", 
                data.candles.iter().map(|c| c.high).fold(0.0f64, f64::max)));
            analysis.push_str(&format!("最低价: {:.2}\n",
                data.candles.iter().map(|c| c.low).fold(f64::MAX, f64::min)));
        }

        analysis
    }

    /// 分析多个市场数据
    pub fn analyze_multiple_data(data_list: &[MarketData]) -> String {
        let mut analysis = String::new();
        analysis.push_str(&format!("分析 {} 个交易对\n\n", data_list.len()));

        for data in data_list.iter().take(5) {
            analysis.push_str(&format!("- {}: {:.2}%\n", data.inst_id, data.price_change_pct));
        }

        if data_list.len() > 5 {
            analysis.push_str(&format!("... 还有 {} 个交易对\n", data_list.len() - 5));
        }

        analysis
    }
}

// ==================== 模式识别器 ====================

/// 模式识别器
///
/// 识别市场数据中的模式和趋势
pub struct PatternRecognizer;

impl PatternRecognizer {
    /// 识别趋势模式
    pub fn recognize_trend(data: &MarketData) -> Option<String> {
        if data.candles.len() < 2 {
            return None;
        }

        let change = data.price_change_pct;

        if change > 10.0 {
            Some(format!("强势上涨 (+{:.1}%)", change))
        } else if change > 5.0 {
            Some(format!("温和上涨 (+{:.1}%)", change))
        } else if change < -10.0 {
            Some(format!("强势下跌 ({:.1}%)", change))
        } else if change < -5.0 {
            Some(format!("温和下跌 ({:.1}%)", change))
        } else {
            Some(format!("横盘震荡 ({:.1}%)", change))
        }
    }

    /// 识别波动性
    pub fn recognize_volatility(data: &MarketData) -> Option<String> {
        if data.candles.len() < 2 {
            return None;
        }

        let volatility: f64 = data.candles.iter()
            .map(|c| ((c.high - c.low) / c.open).abs())
            .sum::<f64>() / data.candles.len() as f64 * 100.0;

        if volatility > 5.0 {
            Some(format!("高波动性 ({:.1}%)", volatility))
        } else if volatility > 2.0 {
            Some(format!("中等波动性 ({:.1}%)", volatility))
        } else {
            Some(format!("低波动性 ({:.1}%)", volatility))
        }
    }

    /// 识别突破模式
    pub fn recognize_breakout(data: &MarketData) -> Option<String> {
        if data.candles.len() < 5 {
            return None;
        }

        let recent_high = data.candles.iter().rev().take(3)
            .map(|c| c.high).fold(0.0f64, f64::max);
        let previous_high = data.candles.iter().rev().skip(3).take(5)
            .map(|c| c.high).fold(0.0f64, f64::max);

        if recent_high > previous_high * 1.02 {
            Some("向上突破".to_string())
        } else if recent_high < previous_high * 0.98 {
            Some("向下突破".to_string())
        } else {
            None
        }
    }
}

// ==================== Discovery Agent ====================

/// 策略发现 Agent
///
/// 负责发现交易机会，生成分析报告
pub struct DiscoveryAgent {
    /// LLM Gateway
    llm_gateway: Arc<LLMGateway>,
    /// 通知渠道
    channels: RwLock<Vec<Arc<dyn Channel>>>,
    /// Agent 名称
    name: String,
}

impl DiscoveryAgent {
    /// 创建新的 Discovery Agent
    pub fn new(llm_gateway: Arc<LLMGateway>) -> Self {
        Self {
            llm_gateway,
            channels: RwLock::new(Vec::new()),
            name: "DiscoveryAgent".to_string(),
        }
    }

    /// 添加通知渠道
    pub async fn add_channel(&self, channel: Arc<dyn Channel>) {
        let mut channels = self.channels.write().await;
        channels.push(channel);
    }

    /// 执行策略发现
    async fn discover_strategy(&self, data: &MarketData) -> Result<AnalysisReport> {
        // 1. 分析市场数据
        let analysis = Analyzer::analyze_market_data(data);

        // 2. 识别模式
        let trend = PatternRecognizer::recognize_trend(data);
        let volatility = PatternRecognizer::recognize_volatility(data);
        let breakout = PatternRecognizer::recognize_breakout(data);

        // 3. 构建提示词
        let prompt = self.build_discovery_prompt(&analysis, &trend, &volatility, &breakout);

        // 4. 调用 LLM 分析
        let messages = vec![
            Message::system("你是一个专业的量化交易策略分析师。"),
            Message::user(prompt),
        ];

        let response = self.llm_gateway.chat(&messages).await?;

        // 5. 解析结果并生成报告
        let report = self.parse_discovery_response(&response.content, data);

        Ok(report)
    }

    /// 执行信号分析
    async fn analyze_signal(&self, signal: &Signal) -> Result<AnalysisReport> {
        let prompt = format!(
            "请分析以下交易信号：\n\n\
            信号ID: {}\n\
            策略: {}\n\
            类型: {:?}\n\
            交易对: {}\n\
            描述: {}\n\
            数据: {}\n\n\
            请提供：\n\
            1. 信号质量评估\n\
            2. 潜在风险点\n\
            3. 执行建议",
            signal.id,
            signal.strategy_name,
            signal.signal_type,
            signal.inst_id,
            signal.description,
            signal.data
        );

        let messages = vec![
            Message::system("你是一个专业的交易信号分析师。"),
            Message::user(prompt),
        ];

        let response = self.llm_gateway.chat(&messages).await?;

        let report = AnalysisReport::new(AgentTaskType::AnalyzeSignal)
            .with_summary(format!("信号分析完成: {}", signal.inst_id))
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
                description: "请仔细审查信号分析结果".to_string(),
                expected_outcome: "做出明智的交易决策".to_string(),
            });

        Ok(report)
    }

    /// 执行模式识别
    async fn recognize_pattern(&self, data_list: &[MarketData]) -> Result<AnalysisReport> {
        let analysis = Analyzer::analyze_multiple_data(data_list);

        let prompt = format!(
            "请分析以下多个交易对的市场数据，识别共同模式：\n\n{}\n\n\
            请识别：\n\
            1. 市场整体趋势\n\
            2. 板块轮动模式\n\
            3. 相关性分析\n\
            4. 潜在套利机会",
            analysis
        );

        let messages = vec![
            Message::system("你是一个专业的市场模式识别专家。"),
            Message::user(prompt),
        ];

        let response = self.llm_gateway.chat(&messages).await?;

        let report = AnalysisReport::new(AgentTaskType::RecognizePattern)
            .with_summary(format!("模式识别完成: 分析了 {} 个交易对", data_list.len()))
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

    /// 构建策略发现提示词
    fn build_discovery_prompt(
        &self,
        analysis: &str,
        trend: &Option<String>,
        volatility: &Option<String>,
        breakout: &Option<String>,
    ) -> String {
        let mut prompt = format!(
            "请分析以下市场数据，发现潜在的交易机会：\n\n{}\n",
            analysis
        );

        if let Some(t) = trend {
            prompt.push_str(&format!("\n趋势识别: {}\n", t));
        }
        if let Some(v) = volatility {
            prompt.push_str(&format!("波动性: {}\n", v));
        }
        if let Some(b) = breakout {
            prompt.push_str(&format!("突破模式: {}\n", b));
        }

        prompt.push_str("\n请提供：\n");
        prompt.push_str("1. 交易机会评估\n");
        prompt.push_str("2. 建议策略类型\n");
        prompt.push_str("3. 风险等级评估\n");
        prompt.push_str("4. 具体操作建议\n");

        prompt
    }

    /// 解析 LLM 响应
    fn parse_discovery_response(&self, response: &str, data: &MarketData) -> AnalysisReport {
        // 简化实现：直接构建报告
        // 实际应用中可以使用 JSON 解析响应
        AnalysisReport::new(AgentTaskType::DiscoverStrategy)
            .with_summary(format!("策略发现: {}", data.inst_id))
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
                description: "持续监控该交易对".to_string(),
                expected_outcome: "捕捉交易机会".to_string(),
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

// ==================== Agent 管理器 ====================

/// Agent 管理器
///
/// 管理所有 Agent 实例，负责任务分发
pub struct AgentManager {
    agents: RwLock<HashMap<String, Arc<dyn Agent>>>,
}

impl AgentManager {
    /// 创建新的 Agent 管理器
    pub fn new() -> Self {
        Self {
            agents: RwLock::new(HashMap::new()),
        }
    }

    /// 注册 Agent
    pub async fn register_agent(&self, name: String, agent: Arc<dyn Agent>) {
        let mut agents = self.agents.write().await;
        agents.insert(name, agent);
    }

    /// 获取 Agent
    pub async fn get_agent(&self, name: &str) -> Option<Arc<dyn Agent>> {
        let agents = self.agents.read().await;
        agents.get(name).cloned()
    }

    /// 异步执行任务（不等待结果）
    pub async fn spawn_task(&self, agent_name: &str, task: AgentTask) {
        let agent = match self.get_agent(agent_name).await {
            Some(a) => a,
            None => {
                eprintln!("Agent '{}' not found", agent_name);
                return;
            }
        };

        // 异步 spawn，不等待结果
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

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;
    use cluebot_engine::{Candle, SignalType};

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
        assert!(trend.unwrap().contains("上涨"));
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
        
        // 创建 mock agent
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
