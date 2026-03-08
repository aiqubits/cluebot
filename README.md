# ClueBot

Quantitative trading signal alerting system — multi-strategy, multi-market, multi-channel collaborative monitoring for assisted manual decision-making.

## Overview

ClueBot is a quantitative trading signal monitoring platform built with Rust. It monitors market data from multiple exchanges through pluggable strategy modules and sends real-time notifications via various channels (email, Lark, etc.) when strategy conditions are triggered.

## Key Features

- **Multi-Strategy Support**: Strategies implemented as independent crates with dynamic loading
- **Multi-Market Access**: Supports major exchanges including OKX, Binance
- **Multi-Channel Notifications**: Email, Lark, and other notification channels
- **AI-Powered Analysis**: Asynchronous Agent tasks for strategy discovery and market analysis
- **Layered Architecture**: Clear separation of concerns for easy extension and maintenance

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│  Application Layer: bin/cluebot (CLI, config, assembly) │
└────────────────────┬────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────────┐
│  Infrastructure Layer: Runtime (Lifecycle Management)   │
└────────────────────▲────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────────┐
│  Business Logic Layer: Engine (Strategy execution)      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐              │
│  │ Executor │  │Scheduler │  │ Monitor  │              │
│  │ Strategy │  │  Task    │  │  Market  │              │
│  │ Execution│  │Scheduler │  │ Monitor  │              │
│  └──────────┘  └──────────┘  └──────────┘              │
└─────────┬──────────────────┬────────────────────────────┘
          │                  │
          ▼                  ▼
┌─────────────────┐  ┌─────────────────┐
│  Strategies     │  │  Extensions     │
│  Strategy       │  │  (Markets)      │
│  Modules        │  │  - OKX          │
│  - Volatility   │  │  - Binance      │
│  - (Extensible) │  │                 │
└─────────────────┘  └─────────────────┘

          │ Signal Triggered
          ▼
┌─────────────────┐  ┌─────────────────┐
│  Extensions     │  │     Agent       │
│  (Channels)     │  │   AI Agent      │
│  - Lark         │  │   Layer         │
│  - Email        │  │  - Discovery    │
│                 │  │  - Analysis     │
└─────────────────┘  └─────────────────┘
```

## Quick Start

### Requirements

- Rust 1.80+
- Exchange API key (for OKX)
- SMTP email configuration (for email notifications)

### Environment Configuration

```bash
cp .env.example .env
# Edit .env file with the following variables:

# SMTP Configuration
SMTP_SERVER=smtp.gmail.com
SMTP_PORT=587
FROM_EMAIL=your-email@gmail.com
SMTP_PASSWORD=your-app-password
SMTP_RECIPIENT=recipient@example.com
USE_TLS=true

# Strategy Parameters
VOLATILITY_PRICE_CHANGE_THRESHOLD=5.0    # Price change threshold (%)
VOLATILITY_VOLATILITY_THRESHOLD=0.0      # Volatility threshold (%)
VOLATILITY_MIN_CANDLES=2                 # Minimum candle count
VOLATILITY_BAR=1H                        # Candle interval
VOLATILITY_LIMIT=8                       # Number of candles
SCAN_INTERVAL_SECS=1800                  # Scan interval (seconds)
```

### Run

```bash
# Development mode
cargo run

# Production build
cargo build --release
./target/release/cluebot
```

## Project Structure

```
cluebot/
├── bin/cluebot/           # Application entry point
├── crates/
│   ├── runtime/           # Infrastructure - Lifecycle management
│   ├── engine/            # Business logic - Strategy execution core
│   ├── agent/             # AI Agent layer
│   │   ├── discovery-strategy/    # Strategy discovery
│   │   ├── strategy-creation/     # Strategy creation
│   │   └── strategy-optimization/ # Strategy optimization
│   ├── llm-gateway/       # AI infrastructure - Unified LLM interface
│   ├── strategies/        # Strategy implementations
│   │   └── volatility-increase-short-selling/  # Volatility short strategy
│   └── extensions/        # Extension modules
│       ├── markets/       # Exchange interfaces
│       │   ├── okx/
│       │   └── binance/
│       └── channels/      # Notification channels
│           ├── email/
│           └── lark/
├── DESIGN.md              # Detailed design documentation
└── README.md              # This document
```

## Developing Strategies

Strategies must implement the `Strategy` trait:

```rust
#[async_trait]
pub trait Strategy: Send + Sync {
    fn name(&self) -> &str;
    async fn execute(&self, market: &dyn Market) -> Result<Vec<Signal>>;
}
```

See example: [Volatility Short Strategy](crates/strategies/volatility-increase-short-selling/src/lib.rs)

## Adding Markets/Channels

Market modules implement the `Market` trait, and channel modules implement the `Channel` trait. Once implemented, they can be dynamically loaded by the Engine.

## Design Documentation

For detailed architecture design, please refer to [DESIGN.md](DESIGN.md)

## License

MIT
