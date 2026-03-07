use anyhow::Result;
use async_trait::async_trait;
use cluebot_engine::Channel;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use serde_json::Value;

/// Email configuration
#[derive(Debug, Clone)]
pub struct EmailConfig {
    /// SMTP server address
    pub smtp_server: String,
    /// SMTP port
    pub smtp_port: u16,
    /// Sender email
    pub from_email: String,
    /// Sender name
    pub from_name: String,
    /// Email account
    pub username: String,
    /// Email password/auth code
    pub password: String,
    /// Whether to use TLS
    pub use_tls: bool,
}

impl EmailConfig {
    /// Create Email configuration
    pub fn email(username: impl Into<String>, password: impl Into<String>) -> Self {
        let username = username.into();
        Self {
            smtp_server: "smtp.gmail.com".to_string(),
            smtp_port: 587,
            from_email: username.clone(),
            from_name: "ClueBot".to_string(),
            username,
            password: password.into(),
            use_tls: true,
        }
    }

    /// Create QQ email configuration
    pub fn qq(username: impl Into<String>, password: impl Into<String>) -> Self {
        let username = username.into();
        Self {
            smtp_server: "smtp.qq.com".to_string(),
            smtp_port: 587,
            from_email: username.clone(),
            from_name: "ClueBot".to_string(),
            username,
            password: password.into(),
            use_tls: true,
        }
    }

    /// Create custom configuration
    /// Scenario 1: Personal email (username = from_email)
    /// Scenario 2: Enterprise email (username ≠ from_email)
    pub fn custom(
        smtp_server: impl Into<String>,
        smtp_port: u16,
        from_email: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
        use_tls: bool,
    ) -> Self {
        Self {
            smtp_server: smtp_server.into(),
            smtp_port,
            from_email: from_email.into(),
            from_name: "ClueBot".to_string(),
            username: username.into(),
            password: password.into(),
            use_tls,
        }
    }
}

/// Email notification channel
pub struct EmailChannel {
    config: EmailConfig,
    transport: Option<AsyncSmtpTransport<Tokio1Executor>>,
    recipients: Vec<String>,
}

impl EmailChannel {
    /// Create new Email channel (not connected immediately)
    pub fn new(config: EmailConfig) -> Self {
        Self {
            config,
            transport: None,
            recipients: Vec::new(),
        }
    }

    /// Add recipient
    pub fn add_recipient(&mut self, email: impl Into<String>) -> &mut Self {
        self.recipients.push(email.into());
        self
    }

    /// Set multiple recipients
    pub fn with_recipients(mut self, emails: Vec<String>) -> Self {
        self.recipients = emails;
        self
    }

    /// Initialize SMTP connection
    pub async fn connect(&mut self) -> Result<()> {
        let creds = Credentials::new(
            self.config.username.clone(),
            self.config.password.clone(),
        );

        let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(
            &self.config.smtp_server,
        )?
        .port(self.config.smtp_port)
        .credentials(creds)
        .build();

        self.transport = Some(transport);
        Ok(())
    }

    /// Send email to all recipients
    async fn send_to_all(&self, subject: &str, body: &str) -> Result<()> {
        let transport: &AsyncSmtpTransport<Tokio1Executor> = match &self.transport {
            Some(t) => t,
            None => return Err(anyhow::anyhow!("Email channel not connected")),
        };

        for recipient in &self.recipients {
            let email = Message::builder()
                .from(
                    format!("{} <{}>", self.config.from_name, self.config.from_email)
                        .parse()?,
                )
                .to(recipient.parse()?)
                .subject(subject)
                .header(ContentType::TEXT_PLAIN)
                .body(body.to_string())?;

            match transport.send(email).await {
                Ok(_) => println!("Email sent to {}", recipient),
                Err(e) => eprintln!("Failed to send email to {}: {}", recipient, e),
            }
        }

        Ok(())
    }

    /// Format notification message as email content
    fn format_email_content(&self, message: &str) -> (String, String) {
        // Try to parse JSON formatted message
        if let Ok(json) = serde_json::from_str::<Value>(message) {
            let subject = format!(
                "[ClueBot] {}",
                json.get("strategy_name")
                    .and_then(|v: &Value| v.as_str())
                    .unwrap_or("Trading Signal")
            );

            let body = format!(
                "ClueBot Trading Alert\n\n{}\n\n---\nGenerated by ClueBot",
                serde_json::to_string_pretty(&json).unwrap_or_else(|_| message.to_string())
            );

            (subject, body)
        } else {
            (
                "[ClueBot] Notification".to_string(),
                format!("{}\n\n---\nGenerated by ClueBot", message),
            )
        }
    }
}

#[async_trait]
impl Channel for EmailChannel {
    fn name(&self) -> &str {
        "Email"
    }

    async fn send(&self, message: &str) -> Result<()> {
        let (subject, body) = self.format_email_content(message);
        self.send_to_all(&subject, &body).await
    }
}

/// Convenience function to create Email channel
pub async fn create_email_channel(
    config: EmailConfig,
    recipients: Vec<String>,
) -> Result<EmailChannel> {
    let mut channel = EmailChannel::new(config).with_recipients(recipients);
    channel.connect().await?;
    Ok(channel)
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_config_gmail() {
        let config = EmailConfig::email("test@gmail.com", "password");
        assert_eq!(config.smtp_server, "smtp.gmail.com");
        assert_eq!(config.smtp_port, 587);
        assert_eq!(config.from_email, "test@gmail.com");
    }

    #[test]
    fn test_email_config_qq() {
        let config = EmailConfig::qq("test@qq.com", "password");
        assert_eq!(config.smtp_server, "smtp.qq.com");
        assert_eq!(config.smtp_port, 587);
    }

    #[test]
    fn test_email_channel_creation() {
        let config = EmailConfig::email("test@gmail.com", "password");
        let channel = EmailChannel::new(config)
            .with_recipients(vec!["recipient@example.com".to_string()]);
        
        assert_eq!(channel.name(), "Email");
        assert_eq!(channel.recipients.len(), 1);
    }

    #[test]
    fn test_format_email_content() {
        let config = EmailConfig::email("test@gmail.com", "password");
        let channel = EmailChannel::new(config);

        // 测试 JSON 消息
        let json_msg = r#"{"strategy_name": "TestStrategy", "signal_type": "Buy"}"#;
        let (subject, body) = channel.format_email_content(json_msg);
        
        assert!(subject.contains("TestStrategy"));
        assert!(body.contains("ClueBot Trading Alert"));
        assert!(body.contains("Buy"));

        // 测试普通消息
        let plain_msg = "Simple notification";
        let (subject, body) = channel.format_email_content(plain_msg);
        
        assert_eq!(subject, "[ClueBot] Notification");
        assert!(body.contains("Simple notification"));
    }
}
