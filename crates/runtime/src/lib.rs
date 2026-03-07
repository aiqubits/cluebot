use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Runtime state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    /// Initialized
    Initialized,
    /// Running
    Running,
    /// Stopped
    Stopped,
}

/// Lifecycle handler trait
///
/// Modules implementing this trait can register to Runtime, which manages their lifecycle uniformly
#[async_trait]
pub trait LifecycleHandler: Send + Sync {
    /// Called on start
    async fn on_start(&self) -> Result<()>;
    /// Called on stop
    async fn on_stop(&self) -> Result<()>;
}

/// Lifecycle manager
///
/// Manages lifecycle of all registered modules, coordinates start and stop processes
pub struct LifecycleManager {
    handlers: RwLock<Vec<Arc<dyn LifecycleHandler>>>,
    state: RwLock<RuntimeState>,
}

impl LifecycleManager {
    /// Create new lifecycle manager
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(Vec::new()),
            state: RwLock::new(RuntimeState::Initialized),
        }
    }

    /// Register lifecycle handler
    ///
    /// # Arguments
    /// * `handler` - Module implementing LifecycleHandler trait
    pub async fn register(&self, handler: Arc<dyn LifecycleHandler>) {
        let mut handlers = self.handlers.write().await;
        handlers.push(handler);
    }

    /// Start all registered modules
    ///
    /// Call each module's on_start method in registration order
    pub async fn start_all(&self) -> Result<()> {
        let handlers = self.handlers.read().await;
        for handler in handlers.iter() {
            handler.on_start().await?;
        }
        let mut state = self.state.write().await;
        *state = RuntimeState::Running;
        Ok(())
    }

    /// Stop all registered modules
    ///
    /// Call each module's on_stop method in reverse registration order
    pub async fn stop_all(&self) -> Result<()> {
        let handlers = self.handlers.read().await;
        // Stop in reverse order to ensure dependency relationships are handled correctly
        for handler in handlers.iter().rev() {
            handler.on_stop().await?;
        }
        let mut state = self.state.write().await;
        *state = RuntimeState::Stopped;
        Ok(())
    }

    /// Get current Runtime state
    pub async fn state(&self) -> RuntimeState {
        *self.state.read().await
    }
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Minimal runtime
///
/// Provides basic lifecycle management functionality, adopts passive service mode
/// Runtime does not actively create upper-level modules, passively receives registration
pub struct Runtime {
    lifecycle: LifecycleManager,
}

impl Runtime {
    /// Create new Runtime instance
    pub fn new() -> Self {
        Self {
            lifecycle: LifecycleManager::new(),
        }
    }

    /// Register lifecycle handler
    ///
    /// # Arguments
    /// * `handler` - Module implementing LifecycleHandler trait
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

    /// Start Runtime
    ///
    /// Trigger on_start callbacks for all registered modules
    pub async fn start(&self) -> Result<()> {
        self.lifecycle.start_all().await
    }

    /// Stop Runtime
    ///
    /// Trigger on_stop callbacks for all registered modules
    pub async fn stop(&self) -> Result<()> {
        self.lifecycle.stop_all().await
    }

    /// Get current Runtime state
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

        // Initial state is Initialized
        assert_eq!(runtime.state().await, RuntimeState::Initialized);

        // Register module
        runtime.register(module.clone()).await;

        // Start Runtime
        runtime.start().await.unwrap();
        assert!(module.is_started().await);
        assert_eq!(runtime.state().await, RuntimeState::Running);

        // Stop Runtime
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
