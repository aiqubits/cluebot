use anyhow::Result;
use cluebot_engine::{Engine, Signal};
use cluebot_okx::OkxMarket;
use cluebot_runtime::Runtime;
use cluebot_email::{EmailConfig, EmailChannel};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use volatility_increase_short_selling::VolatilityIncreaseShortSellingStrategy;

/// Process signals
async fn process_signals(engine: &Engine, signals: Vec<Signal>) -> Result<()> {
    if signals.is_empty() {
        println!("No short-selling opportunities found");
        return Ok(());
    }

    println!("\n========================================");
    println!("  Short-selling Signal List");
    println!("========================================");
    
    for signal in &signals {
        println!("\nSignal ID: {}", signal.id);
        println!("Trading Pair: {}", signal.inst_id);
        println!("Description: {}", signal.description);
        println!("Time: {}", signal.created_at);
        
        // Send notification
        engine.send_notification(signal).await?;
    }
    
    println!("\nFound {} short-selling opportunities", signals.len());
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("========================================");
    println!("  ClueBot - Volatility Short-selling Strategy Scanner");
    println!("========================================\n");

    // 1. Create Runtime
    let runtime = Arc::new(Runtime::new());

    // 2. Create Engine
    let engine = Arc::new(Engine::new());

    // 3. Register Engine to Runtime
    runtime.register(engine.clone()).await;

    // 4. Load environment variables
    dotenvy::dotenv().ok();
    
    // 5. Read email config from environment variables
    let smtp_server = env::var("SMTP_SERVER")
        .map_err(|_| anyhow::anyhow!("Environment variable SMTP_SERVER not set"))?;
    let smtp_port = env::var("SMTP_PORT")
        .unwrap_or_else(|_| "587".to_string()).parse::<u16>()?;
    let from_email = env::var("FROM_EMAIL")
        .map_err(|_| anyhow::anyhow!("Environment variable FROM_EMAIL not set"))?;
    let smtp_username = env::var("SMTP_USERNAME")
        .unwrap_or_else(|_| from_email.clone());
    let smtp_password = env::var("SMTP_PASSWORD")
        .map_err(|_| anyhow::anyhow!("Environment variable SMTP_PASSWORD not set"))?;
    let smtp_recipient = env::var("SMTP_RECIPIENT")
        .unwrap_or_else(|_| smtp_username.clone());
    let use_tls = env::var("USE_TLS")
        .unwrap_or_else(|_| "true".to_string()).parse::<bool>()?;
    
    // 6. Create email config
    let email_config = EmailConfig::custom(smtp_server, smtp_port, from_email, smtp_username, smtp_password, use_tls);
    
    // 7. Create email channel and connect
    let mut email_channel = EmailChannel::new(email_config)
        .with_recipients(vec![smtp_recipient]);
    email_channel.connect().await?;
    
    // 8. Add email_channel to Engine
    engine.add_channel(Arc::new(email_channel)).await?;

    // 9. Create OKX market client
    let okx = Arc::new(OkxMarket::new());

    // 10. Load volatility short-selling strategy config from environment variables
    let price_change_threshold: f64 = env::var("VOLATILITY_PRICE_CHANGE_THRESHOLD")
        .unwrap_or_else(|_| "5.0".to_string()).parse::<f64>()?;
    let volatility_threshold: f64 = env::var("VOLATILITY_VOLATILITY_THRESHOLD")
        .unwrap_or_else(|_| "0.0".to_string()).parse::<f64>()?;
    let min_candles: usize = env::var("VOLATILITY_MIN_CANDLES")
        .unwrap_or_else(|_| "2".to_string()).parse::<usize>()?;
    let bar: String = env::var("VOLATILITY_BAR")
        .unwrap_or_else(|_| "1H".to_string());
    let limit: u32 = env::var("VOLATILITY_LIMIT")
        .unwrap_or_else(|_| "8".to_string()).parse::<u32>()?;
    let max_coins_to_check: usize = env::var("VOLATILITY_MAX_COINS_TO_CHECK")
        .unwrap_or_else(|_| "0".to_string()).parse::<usize>()?;
    let strategy = Arc::new(VolatilityIncreaseShortSellingStrategy::new(
        volatility_increase_short_selling::VolatilityStrategyConfig {
            price_change_threshold,
            volatility_threshold,
            min_candles,
            bar: bar.clone(),
            limit,
            max_coins_to_check,
        }
    ));
    engine.load_strategy(strategy).await?;
    println!("Strategy loaded: VolatilityIncreaseShortSelling");
    println!("  Price Change Threshold: {}%", price_change_threshold);
    println!("  Volatility Threshold: {}%", volatility_threshold);
    println!("  Min Candles: {}", min_candles);
    println!("  Bar: {}", bar);
    println!("  Limit: {}", limit);
    println!("  Max Coins To Check: {} (0 means all)", max_coins_to_check);

    // 11. Start Runtime
    runtime.start().await?;
    println!("Engine started\n");

    // 12. Use Engine scheduler to execute strategy periodically
    let scan_interval_secs: u64 = env::var("SCAN_INTERVAL_SECS")
        .unwrap_or_else(|_| "1800".to_string()).parse::<u64>()?;
    engine.schedule_repeating(
        cluebot_engine::TaskType::CheckConditions,
        Duration::from_secs(scan_interval_secs), // Execute every scan_interval_secs seconds
        {
            let engine = engine.clone();
            let okx = okx.clone();
            
            move || {
                let engine = engine.clone();
                let okx = okx.clone();
                
                async move {
                    println!("\n[{}] Starting strategy scan...", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
                    
                    // Strategy fetches data and executes autonomously
                    let signals = engine.execute_strategies(&*okx).await?;
                    
                    // Process signals
                    process_signals(&engine, signals).await?;
                    
                    Ok(())
                }
            }
        }
    ).await?;

    println!("Scheduler started, scanning market every 60 seconds\n");
    println!("Press Ctrl+C to stop the program...\n");

    // 13. Wait for exit signal
    tokio::signal::ctrl_c().await?;
    println!("\nReceived exit signal...");

    // 14. Stop Runtime
    runtime.stop().await?;
    println!("Engine stopped");

    Ok(())
}
