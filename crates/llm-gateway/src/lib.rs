use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

// ==================== 类型定义 (types.rs) ====================

/// 消息角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// 聊天消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
        }
    }
}

/// LLM 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    /// 生成的内容
    pub content: String,
    /// 使用的 token 数（输入）
    pub prompt_tokens: Option<u32>,
    /// 使用的 token 数（输出）
    pub completion_tokens: Option<u32>,
    /// 总 token 数
    pub total_tokens: Option<u32>,
    /// 模型名称
    pub model: String,
    /// 响应时间
    pub response_time_ms: u64,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

/// 消息事件（用于流式响应）
#[derive(Debug, Clone)]
pub enum MessageEvent {
    /// 增量内容
    Delta(String),
    /// 完成
    Done,
    /// 错误
    Error(String),
}

/// LLM 提供商类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderType {
    OpenAI,
    Anthropic,
    Custom(&'static str),
}

impl ProviderType {
    pub fn as_str(&self) -> &str {
        match self {
            ProviderType::OpenAI => "openai",
            ProviderType::Anthropic => "anthropic",
            ProviderType::Custom(s) => s,
        }
    }
}

// ==================== Provider trait (client.rs) ====================

/// LLM 提供商 trait
///
/// OpenAI、Anthropic 等提供商实现此 trait
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// 获取提供商名称
    fn name(&self) -> &str;

    /// 发送聊天请求
    ///
    /// # Arguments
    /// * `messages` - 消息列表
    ///
    /// # Returns
    /// * `Ok(LLMResponse)` - LLM 响应
    async fn chat(&self, messages: &[Message]) -> Result<LLMResponse>;

    /// 发送聊天请求（流式响应）
    ///
    /// # Arguments
    /// * `messages` - 消息列表
    ///
    /// # Returns
    /// * `Ok(Receiver<MessageEvent>)` - 消息事件接收器
    async fn chat_with_stream(
        &self,
        messages: &[Message],
    ) -> Result<mpsc::Receiver<MessageEvent>>;
}

// ==================== 提示词模板管理 (prompt.rs) ====================

/// 提示词模板
#[derive(Debug, Clone)]
pub struct PromptTemplate {
    /// 模板名称
    pub name: String,
    /// 模板内容（包含变量占位符，如 {{variable}}）
    pub template: String,
    /// 描述
    pub description: Option<String>,
    /// 版本
    pub version: String,
}

impl PromptTemplate {
    /// 创建新模板
    pub fn new(name: impl Into<String>, template: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            template: template.into(),
            description: None,
            version: "1.0.0".to_string(),
        }
    }

    /// 设置描述
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// 设置版本
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// 渲染模板
    ///
    /// # Arguments
    /// * `variables` - 变量映射
    ///
    /// # Returns
    /// * `Ok(String)` - 渲染后的提示词
    pub fn render(&self, variables: &HashMap<String, String>) -> Result<String> {
        let mut result = self.template.clone();
        for (key, value) in variables.iter() {
            let placeholder = format!("{{{{{}}}}}", key);
            result = result.replace(&placeholder, value);
        }
        Ok(result)
    }
}

/// 提示词管理器
pub struct PromptManager {
    templates: RwLock<HashMap<String, PromptTemplate>>,
}

impl PromptManager {
    /// 创建新的提示词管理器
    pub fn new() -> Self {
        Self {
            templates: RwLock::new(HashMap::new()),
        }
    }

    /// 注册模板
    pub async fn register_template(&self, template: PromptTemplate) {
        let mut templates = self.templates.write().await;
        templates.insert(template.name.clone(), template);
    }

    /// 获取模板
    pub async fn get_template(&self, name: &str) -> Option<PromptTemplate> {
        let templates = self.templates.read().await;
        templates.get(name).cloned()
    }

    /// 渲染指定模板
    pub async fn render_template(
        &self,
        name: &str,
        variables: &HashMap<String, String>,
    ) -> Result<String> {
        let template = self
            .get_template(name)
            .await
            .ok_or_else(|| anyhow::anyhow!("Template '{}' not found", name))?;
        template.render(variables)
    }

    /// 创建系统提示词（策略发现）
    pub fn create_strategy_discovery_prompt() -> PromptTemplate {
        PromptTemplate::new(
            "strategy_discovery",
            r#"你是一个量化交易策略分析专家。

请分析以下市场数据，识别潜在的交易机会：

市场数据：
{{market_data}}

请从以下角度分析：
1. 市场趋势识别
2. 波动率异常检测
3. 潜在交易机会
4. 风险评估

请以 JSON 格式返回分析结果：
{
    "opportunities": [
        {
            "type": "趋势跟踪/均值回归/突破",
            "confidence": 0.85,
            "description": "描述",
            "suggested_action": "建议操作"
        }
    ],
    "risk_level": "low/medium/high",
    "summary": "分析摘要"
}"#,
        )
        .with_description("策略发现分析提示词")
    }

    /// 创建系统提示词（信号分析）
    pub fn create_signal_analysis_prompt() -> PromptTemplate {
        PromptTemplate::new(
            "signal_analysis",
            r#"你是一个交易信号分析专家。

请分析以下交易信号：

信号信息：
{{signal_data}}

历史上下文：
{{context}}

请分析：
1. 信号质量评估
2. 历史相似情况回顾
3. 潜在风险点
4. 执行建议

请提供结构化的分析报告。"#,
        )
        .with_description("交易信号分析提示词")
    }
}

impl Default for PromptManager {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== LLM Gateway 核心 ====================

/// LLM Gateway - AI 基础设施层
///
/// 统一 LLM 访问接口，解耦 Agent 与具体 LLM 提供商
pub struct LLMGateway {
    /// 注册的提供商
    providers: RwLock<HashMap<String, Arc<dyn LLMProvider>>>,
    /// 默认提供商名称
    default_provider: RwLock<Option<String>>,
    /// 提示词管理器
    prompt_manager: PromptManager,
}

impl LLMGateway {
    /// 创建新的 LLM Gateway
    pub fn new() -> Self {
        let gateway = Self {
            providers: RwLock::new(HashMap::new()),
            default_provider: RwLock::new(None),
            prompt_manager: PromptManager::new(),
        };

        // 初始化默认提示词模板
        tokio::spawn(async move {
            // 这里会在运行时初始化
        });

        gateway
    }

    /// 注册提供商
    ///
    /// # Arguments
    /// * `name` - 提供商名称
    /// * `provider` - 提供商实例
    pub async fn register_provider(&self, name: String, provider: Arc<dyn LLMProvider>) {
        let mut providers = self.providers.write().await;
        providers.insert(name.clone(), provider);

        // 如果是第一个注册的提供商，设为默认
        let mut default = self.default_provider.write().await;
        if default.is_none() {
            *default = Some(name);
        }
    }

    /// 设置默认提供商
    pub async fn set_default_provider(&self, name: &str) -> Result<()> {
        let providers = self.providers.read().await;
        if !providers.contains_key(name) {
            return Err(anyhow::anyhow!("Provider '{}' not registered", name));
        }
        drop(providers);

        let mut default = self.default_provider.write().await;
        *default = Some(name.to_string());
        Ok(())
    }

    /// 获取默认提供商名称
    pub async fn default_provider(&self) -> Option<String> {
        self.default_provider.read().await.clone()
    }

    /// 发送聊天请求（使用默认提供商）
    pub async fn chat(&self, messages: &[Message]) -> Result<LLMResponse> {
        let provider_name = self
            .default_provider()
            .await
            .ok_or_else(|| anyhow::anyhow!("No default provider set"))?;
        self.chat_with_provider(&provider_name, messages).await
    }

    /// 发送聊天请求（指定提供商）
    pub async fn chat_with_provider(
        &self,
        provider_name: &str,
        messages: &[Message],
    ) -> Result<LLMResponse> {
        let providers = self.providers.read().await;
        let provider = providers
            .get(provider_name)
            .ok_or_else(|| anyhow::anyhow!("Provider '{}' not found", provider_name))?;
        provider.chat(messages).await
    }

    /// 流式聊天请求（使用默认提供商）
    pub async fn chat_with_stream(
        &self,
        messages: &[Message],
    ) -> Result<mpsc::Receiver<MessageEvent>> {
        let provider_name = self
            .default_provider()
            .await
            .ok_or_else(|| anyhow::anyhow!("No default provider set"))?;
        self.chat_with_stream_and_provider(&provider_name, messages).await
    }

    /// 流式聊天请求（指定提供商）
    pub async fn chat_with_stream_and_provider(
        &self,
        provider_name: &str,
        messages: &[Message],
    ) -> Result<mpsc::Receiver<MessageEvent>> {
        let providers = self.providers.read().await;
        let provider = providers
            .get(provider_name)
            .ok_or_else(|| anyhow::anyhow!("Provider '{}' not found", provider_name))?;
        provider.chat_with_stream(messages).await
    }

    /// 使用模板发送请求
    pub async fn chat_with_template(
        &self,
        template_name: &str,
        variables: &HashMap<String, String>,
    ) -> Result<LLMResponse> {
        let prompt = self
            .prompt_manager
            .render_template(template_name, variables)
            .await?;
        let messages = vec![Message::user(prompt)];
        self.chat(&messages).await
    }

    /// 获取提示词管理器
    pub fn prompt_manager(&self) -> &PromptManager {
        &self.prompt_manager
    }
}

impl Default for LLMGateway {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    struct MockProvider {
        name: String,
    }

    #[async_trait]
    impl LLMProvider for MockProvider {
        fn name(&self) -> &str {
            &self.name
        }

        async fn chat(&self, _messages: &[Message]) -> Result<LLMResponse> {
            Ok(LLMResponse {
                content: "Mock response".to_string(),
                prompt_tokens: Some(10),
                completion_tokens: Some(5),
                total_tokens: Some(15),
                model: "mock-model".to_string(),
                response_time_ms: 100,
                created_at: Utc::now(),
            })
        }

        async fn chat_with_stream(
            &self,
            _messages: &[Message],
        ) -> Result<mpsc::Receiver<MessageEvent>> {
            let (tx, rx) = mpsc::channel(10);
            tokio::spawn(async move {
                let _ = tx.send(MessageEvent::Delta("Mock ".to_string())).await;
                let _ = tx.send(MessageEvent::Delta("response".to_string())).await;
                let _ = tx.send(MessageEvent::Done).await;
            });
            Ok(rx)
        }
    }

    #[tokio::test]
    async fn test_llm_gateway_basic() {
        let gateway = LLMGateway::new();

        // 注册提供商
        gateway
            .register_provider("mock".to_string(), Arc::new(MockProvider {
                name: "mock".to_string(),
            }))
            .await;

        // 测试聊天
        let messages = vec![Message::user("Hello")];
        let response = gateway.chat(&messages).await.unwrap();
        assert_eq!(response.content, "Mock response");
    }

    #[tokio::test]
    async fn test_prompt_template() {
        let template = PromptTemplate::new(
            "test",
            "Hello {{name}}, welcome to {{place}}!",
        );

        let mut variables = HashMap::new();
        variables.insert("name".to_string(), "Alice".to_string());
        variables.insert("place".to_string(), "Wonderland".to_string());

        let result = template.render(&variables).unwrap();
        assert_eq!(result, "Hello Alice, welcome to Wonderland!");
    }

    #[tokio::test]
    async fn test_prompt_manager() {
        let manager = PromptManager::new();
        let template = PromptTemplate::new("greeting", "Hello {{name}}!");

        manager.register_template(template).await;

        let mut variables = HashMap::new();
        variables.insert("name".to_string(), "World".to_string());

        let result = manager.render_template("greeting", &variables).await.unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[tokio::test]
    async fn test_message_creation() {
        let system_msg = Message::system("You are a helpful assistant");
        assert_eq!(system_msg.role, MessageRole::System);

        let user_msg = Message::user("Hello");
        assert_eq!(user_msg.role, MessageRole::User);

        let assistant_msg = Message::assistant("Hi there");
        assert_eq!(assistant_msg.role, MessageRole::Assistant);
    }
}
