# ClueBot

量化交易信号提醒系统 —— 多策略、多市场、多渠道协同监控，辅助人工决策。

## 简介

ClueBot 是一个基于 Rust 开发的量化交易信号监控平台。它通过可插拔的策略模块监控多个交易所的市场数据，当策略条件触发时，通过多种渠道（邮件、飞书等）发送实时通知。

## 核心特性

- **多策略支持**：策略以独立 crate 形式实现，支持动态加载
- **多市场接入**：支持 OKX、Binance 等主流交易所
- **多渠道通知**：支持邮件、飞书等多种通知方式
- **AI 辅助分析**：异步 Agent 任务进行策略发现和市场分析
- **分层架构**：清晰的职责分离，便于扩展和维护

## 架构概览

```
┌─────────────────────────────────────────────────────────┐
│  应用层: bin/cluebot (CLI入口、配置加载、组件组装)       │
└────────────────────┬────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────────┐
│  基础设施层: Runtime (生命周期管理)                      │
└────────────────────▲────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────────┐
│  业务逻辑层: Engine (策略执行、市场监控、信号生成)       │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐              │
│  │ Executor │  │Scheduler │  │ Monitor  │              │
│  │ 策略执行 │  │ 任务调度 │  │ 市场监控 │              │
│  └──────────┘  └──────────┘  └──────────┘              │
└─────────┬──────────────────┬────────────────────────────┘
          │                  │
          ▼                  ▼
┌─────────────────┐  ┌─────────────────┐
│  Strategies     │  │  Extensions     │
│  策略模块        │  │  (Markets)      │
│  - 波动率做空   │  │  - OKX          │
│  - (可扩展)     │  │  - Binance      │
└─────────────────┘  └─────────────────┘

          │ 触发信号
          ▼
┌─────────────────┐  ┌─────────────────┐
│  Extensions     │  │     Agent       │
│  (Channels)     │  │   AI代理层      │
│  - Lark         │  │  - 策略发现     │
│  - Email        │  │  - 异步分析     │
└─────────────────┘  └─────────────────┘
```

## 快速开始

### 环境要求

- Rust 1.80+
- 交易所 API 密钥（如使用 OKX）
- SMTP 邮箱配置（用于邮件通知）

### 配置环境变量

```bash
cp .env.example .env
# 编辑 .env 文件，配置以下变量：

# SMTP 邮件配置
SMTP_SERVER=smtp.gmail.com
SMTP_PORT=587
FROM_EMAIL=your-email@gmail.com
SMTP_PASSWORD=your-app-password
SMTP_RECIPIENT=recipient@example.com
USE_TLS=true

# 策略参数
VOLATILITY_PRICE_CHANGE_THRESHOLD=5.0    # 价格涨幅阈值(%)
VOLATILITY_VOLATILITY_THRESHOLD=0.0      # 波动率阈值(%)
VOLATILITY_MIN_CANDLES=2                 # 最小K线数量
VOLATILITY_BAR=1H                        # K线周期
VOLATILITY_LIMIT=8                       # K线数量
SCAN_INTERVAL_SECS=1800                  # 扫描间隔(秒)
```

### 运行

```bash
# 开发模式运行
cargo run

# 生产模式构建
cargo build --release
./target/release/cluebot
```

## 项目结构

```
cluebot/
├── bin/cluebot/           # 应用程序入口
├── crates/
│   ├── runtime/           # 基础设施层 - 生命周期管理
│   ├── engine/            # 业务逻辑层 - 策略执行核心
│   ├── agent/             # AI代理层
│   │   ├── discovery-strategy/    # 策略发现
│   │   ├── strategy-creation/     # 策略创建
│   │   └── strategy-optimization/ # 策略优化
│   ├── llm-gateway/       # AI基础设施 - LLM统一接口
│   ├── strategies/        # 策略实现
│   │   └── volatility-increase-short-selling/  # 波动率做空策略
│   └── extensions/        # 扩展模块
│       ├── markets/       # 交易所接口
│       │   ├── okx/
│       │   └── binance/
│       └── channels/      # 通知渠道
│           ├── email/
│           └── lark/
├── DESIGN.md              # 详细设计文档
└── README.md              # 本文档
```

## 开发策略

策略需要实现 `Strategy` trait：

```rust
#[async_trait]
pub trait Strategy: Send + Sync {
    fn name(&self) -> &str;
    async fn execute(&self, market: &dyn Market) -> Result<Vec<Signal>>;
}
```

参考示例：[波动率做空策略](crates/strategies/volatility-increase-short-selling/src/lib.rs)

## 添加市场/渠道

市场模块实现 `Market` trait，渠道模块实现 `Channel` trait，即可被 Engine 动态加载。

## 设计文档

详细架构设计请参考 [DESIGN.md](DESIGN.md)

## License

MIT
