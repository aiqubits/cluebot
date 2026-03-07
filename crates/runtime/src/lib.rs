use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Runtime 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    /// 已初始化
    Initialized,
    /// 运行中
    Running,
    /// 已停止
    Stopped,
}

/// 生命周期处理器 trait
///
/// 实现此 trait 的模块可以注册到 Runtime，由 Runtime 统一管理其生命周期
#[async_trait]
pub trait LifecycleHandler: Send + Sync {
    /// 启动时调用
    async fn on_start(&self) -> Result<()>;
    /// 停止时调用
    async fn on_stop(&self) -> Result<()>;
}

/// 生命周期管理器
///
/// 负责管理所有已注册模块的生命周期，协调启动和停止流程
pub struct LifecycleManager {
    handlers: RwLock<Vec<Arc<dyn LifecycleHandler>>>,
    state: RwLock<RuntimeState>,
}

impl LifecycleManager {
    /// 创建新的生命周期管理器
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(Vec::new()),
            state: RwLock::new(RuntimeState::Initialized),
        }
    }

    /// 注册生命周期处理器
    ///
    /// # Arguments
    /// * `handler` - 实现了 LifecycleHandler trait 的模块
    pub async fn register(&self, handler: Arc<dyn LifecycleHandler>) {
        let mut handlers = self.handlers.write().await;
        handlers.push(handler);
    }

    /// 启动所有已注册的模块
    ///
    /// 按注册顺序依次调用每个模块的 on_start 方法
    pub async fn start_all(&self) -> Result<()> {
        let handlers = self.handlers.read().await;
        for handler in handlers.iter() {
            handler.on_start().await?;
        }
        let mut state = self.state.write().await;
        *state = RuntimeState::Running;
        Ok(())
    }

    /// 停止所有已注册的模块
    ///
    /// 按注册逆序依次调用每个模块的 on_stop 方法
    pub async fn stop_all(&self) -> Result<()> {
        let handlers = self.handlers.read().await;
        // 逆序停止，确保依赖关系正确处理
        for handler in handlers.iter().rev() {
            handler.on_stop().await?;
        }
        let mut state = self.state.write().await;
        *state = RuntimeState::Stopped;
        Ok(())
    }

    /// 获取当前 Runtime 状态
    pub async fn state(&self) -> RuntimeState {
        *self.state.read().await
    }
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 最小化运行时
///
/// 提供基础的生命周期管理功能，采用被动服务模式
/// Runtime 不主动创建上层模块，被动接收注册
pub struct Runtime {
    lifecycle: LifecycleManager,
}

impl Runtime {
    /// 创建新的 Runtime 实例
    pub fn new() -> Self {
        Self {
            lifecycle: LifecycleManager::new(),
        }
    }

    /// 注册生命周期处理器
    ///
    /// # Arguments
    /// * `handler` - 实现了 LifecycleHandler trait 的模块
    ///
    /// # Example
    /// ```rust
    /// use std::sync::Arc;
    /// use cluebot_runtime::{Runtime, LifecycleHandler};
    /// use anyhow::Result;
    ///
    /// struct MyModule;
    ///
    /// #[async_trait::async_trait]
    /// impl LifecycleHandler for MyModule {
    ///     async fn on_start(&self) -> Result<()> {
    ///         println!("MyModule started");
    ///         Ok(())
    ///     }
    ///     async fn on_stop(&self) -> Result<()> {
    ///         println!("MyModule stopped");
    ///         Ok(())
    ///     }
    /// }
    ///
    /// async fn example() {
    ///     let runtime = Runtime::new();
    ///     let module = Arc::new(MyModule);
    ///     runtime.register(module).await;
    /// }
    /// ```
    pub async fn register(&self, handler: Arc<dyn LifecycleHandler>) {
        self.lifecycle.register(handler).await;
    }

    /// 启动 Runtime
    ///
    /// 触发所有已注册模块的 on_start 回调
    pub async fn start(&self) -> Result<()> {
        self.lifecycle.start_all().await
    }

    /// 停止 Runtime
    ///
    /// 触发所有已注册模块的 on_stop 回调
    pub async fn stop(&self) -> Result<()> {
        self.lifecycle.stop_all().await
    }

    /// 获取当前 Runtime 状态
    pub async fn state(&self) -> RuntimeState {
        self.lifecycle.state().await
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestModule {
        name: String,
        started: RwLock<bool>,
        stopped: RwLock<bool>,
    }

    impl TestModule {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                started: RwLock::new(false),
                stopped: RwLock::new(false),
            }
        }

        async fn is_started(&self) -> bool {
            *self.started.read().await
        }

        async fn is_stopped(&self) -> bool {
            *self.stopped.read().await
        }
    }

    #[async_trait]
    impl LifecycleHandler for TestModule {
        async fn on_start(&self) -> Result<()> {
            let mut started = self.started.write().await;
            *started = true;
            println!("{} started", self.name);
            Ok(())
        }

        async fn on_stop(&self) -> Result<()> {
            let mut stopped = self.stopped.write().await;
            *stopped = true;
            println!("{} stopped", self.name);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_runtime_lifecycle() {
        let runtime = Runtime::new();
        let module = Arc::new(TestModule::new("TestModule"));

        // 初始状态为 Initialized
        assert_eq!(runtime.state().await, RuntimeState::Initialized);

        // 注册模块
        runtime.register(module.clone()).await;

        // 启动 Runtime
        runtime.start().await.unwrap();
        assert!(module.is_started().await);
        assert_eq!(runtime.state().await, RuntimeState::Running);

        // 停止 Runtime
        runtime.stop().await.unwrap();
        assert!(module.is_stopped().await);
        assert_eq!(runtime.state().await, RuntimeState::Stopped);
    }

    #[tokio::test]
    async fn test_multiple_modules() {
        let runtime = Runtime::new();
        let module1 = Arc::new(TestModule::new("Module1"));
        let module2 = Arc::new(TestModule::new("Module2"));

        runtime.register(module1.clone()).await;
        runtime.register(module2.clone()).await;

        runtime.start().await.unwrap();
        assert!(module1.is_started().await);
        assert!(module2.is_started().await);

        runtime.stop().await.unwrap();
        assert!(module1.is_stopped().await);
        assert!(module2.is_stopped().await);
    }
}
