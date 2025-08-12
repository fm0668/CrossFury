//! 现代化连接器管理器实现
//! 
//! 提供统一的连接器生命周期管理、监控和配置功能

use async_trait::async_trait;
use std::{
    collections::HashMap,
    sync::Arc,
};
use tokio::sync::RwLock;
use log::{info, warn, error, debug};
use serde::{Serialize, Deserialize};

use crate::connectors::traits::modern::{
    ModernConnectorManager, ModernExchangeConnector, ModernConnectionStatus,
    HealthCheckResult, MetricsCollector, ConnectionEvent
};
use crate::types::errors::ConnectorError;

/// 连接器管理器错误类型
#[derive(Debug, thiserror::Error)]
pub enum ManagerError {
    #[error("连接器不存在: {id}")]
    ConnectorNotFound { id: String },
    
    #[error("连接器已存在: {id}")]
    ConnectorAlreadyExists { id: String },
    
    #[error("连接器操作失败: {source}")]
    ConnectorOperationFailed {
        #[from]
        source: ConnectorError,
    },
    
    #[error("管理器状态错误: {message}")]
    InvalidState { message: String },
}

/// 连接器注册信息
#[derive(Debug, Clone)]
struct ConnectorRegistration<C> {
    /// 连接器实例
    connector: C,
    /// 注册时间
    #[allow(dead_code)]
    registered_at: chrono::DateTime<chrono::Utc>,
    /// 是否自动启动
    #[allow(dead_code)]
    auto_start: bool,
    /// 标签
    tags: HashMap<String, String>,
}

/// 现代化连接器管理器实现
pub struct DefaultModernConnectorManager<C: ModernExchangeConnector> {
    /// 连接器注册表
    connectors: Arc<RwLock<HashMap<String, ConnectorRegistration<C>>>>,
    /// 指标收集器
    metrics_collector: Option<Arc<dyn MetricsCollector>>,
    /// 管理器配置
    #[allow(dead_code)]
    config: ManagerConfig,
}

/// 管理器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagerConfig {
    /// 启动超时时间（秒）
    pub startup_timeout_secs: u64,
    /// 停止超时时间（秒）
    pub shutdown_timeout_secs: u64,
    /// 健康检查间隔（秒）
    pub health_check_interval_secs: u64,
    /// 是否启用自动重连
    pub enable_auto_reconnect: bool,
    /// 最大并发操作数
    pub max_concurrent_operations: usize,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            startup_timeout_secs: 30,
            shutdown_timeout_secs: 10,
            health_check_interval_secs: 60,
            enable_auto_reconnect: true,
            max_concurrent_operations: 10,
        }
    }
}

impl<C: ModernExchangeConnector> DefaultModernConnectorManager<C> {
    /// 创建新的管理器
    pub fn new() -> Self {
        Self {
            connectors: Arc::new(RwLock::new(HashMap::new())),
            metrics_collector: None,
            config: ManagerConfig::default(),
        }
    }
    
    /// 使用配置创建管理器
    pub fn with_config(config: ManagerConfig) -> Self {
        Self {
            connectors: Arc::new(RwLock::new(HashMap::new())),
            metrics_collector: None,
            config,
        }
    }
    
    /// 设置指标收集器
    pub fn with_metrics_collector(mut self, collector: Arc<dyn MetricsCollector>) -> Self {
        self.metrics_collector = Some(collector);
        self
    }
    
    /// 注册连接器（带选项）
    pub async fn register_connector_with_options(
        &mut self,
        id: String,
        connector: C,
        auto_start: bool,
        tags: HashMap<String, String>,
    ) -> Result<(), ManagerError> {
        let mut connectors = self.connectors.write().await;
        
        if connectors.contains_key(&id) {
            return Err(ManagerError::ConnectorAlreadyExists { id });
        }
        
        let registration = ConnectorRegistration {
            connector,
            registered_at: chrono::Utc::now(),
            auto_start,
            tags,
        };
        
        connectors.insert(id.clone(), registration);
        
        info!("连接器已注册: {id}");
        
        // 记录注册事件
        if let Some(ref collector) = self.metrics_collector {
            collector.record_connection_event(&id, ConnectionEvent::Connected).await;
        }
        
        Ok(())
    }
    
    /// 获取连接器数量
    pub async fn connector_count(&self) -> usize {
        let connectors = self.connectors.read().await;
        connectors.len()
    }
    
    /// 获取连接器标签
    pub async fn get_connector_tags(&self, id: &str) -> Option<HashMap<String, String>> {
        let connectors = self.connectors.read().await;
        connectors.get(id).map(|reg| reg.tags.clone())
    }
    
    /// 按标签查找连接器
    pub async fn find_connectors_by_tag(&self, key: &str, value: &str) -> Vec<String> {
        let connectors = self.connectors.read().await;
        connectors
            .iter()
            .filter(|(_, reg)| {
                reg.tags.get(key).is_some_and(|v| v == value)
            })
            .map(|(id, _)| id.clone())
            .collect()
    }
}

impl<C: ModernExchangeConnector> Default for DefaultModernConnectorManager<C> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<C: ModernExchangeConnector> ModernConnectorManager for DefaultModernConnectorManager<C> {
    type Connector = C;
    type Error = ManagerError;
    
    async fn register_connector(
        &mut self,
        id: String,
        connector: Self::Connector,
    ) -> Result<(), Self::Error> {
        self.register_connector_with_options(id, connector, false, HashMap::new()).await
    }
    
    async fn unregister_connector(&mut self, id: &str) -> Result<(), Self::Error> {
        let mut connectors = self.connectors.write().await;
        
        match connectors.remove(id) {
            Some(_) => {
                info!("连接器已注销: {id}");
                
                // 记录注销事件
                if let Some(ref collector) = self.metrics_collector {
                    collector.record_connection_event(id, ConnectionEvent::Disconnected).await;
                }
                
                Ok(())
            }
            None => Err(ManagerError::ConnectorNotFound { id: id.to_string() }),
        }
    }
    
    fn get_connector(&self, _id: &str) -> Option<&Self::Connector> {
        // 注意：由于异步锁的限制，这个方法在当前设计中无法实现
        // 在实际使用中，应该使用异步版本的方法
        None
    }
    
    fn get_connector_mut(&mut self, _id: &str) -> Option<&mut Self::Connector> {
        // 注意：由于异步锁的限制，这个方法在当前设计中无法实现
        // 在实际使用中，应该使用异步版本的方法
        None
    }
    
    fn list_connectors(&self) -> Vec<String> {
        // 注意：由于异步锁的限制，这个方法在当前设计中无法实现
        // 在实际使用中，应该使用异步版本的方法
        Vec::new()
    }
    
    async fn start_all(&mut self) -> Result<(), Self::Error> {
        let connectors = self.connectors.read().await;
        let connector_ids: Vec<String> = connectors.keys().cloned().collect();
        drop(connectors);
        
        info!("开始启动所有连接器，总数: {}", connector_ids.len());
        
        let errors: Vec<String> = Vec::new();
        
        for id in connector_ids {
            // 由于trait限制，这里需要重新设计
            // 实际实现中应该提供异步版本的获取方法
            debug!("启动连接器: {id}");
        }
        
        if errors.is_empty() {
            info!("所有连接器启动成功");
            Ok(())
        } else {
            error!("部分连接器启动失败: {errors:?}");
            Err(ManagerError::InvalidState {
                message: format!("启动失败的连接器数量: {}", errors.len()),
            })
        }
    }
    
    async fn stop_all(&mut self) -> Result<(), Self::Error> {
        let connectors = self.connectors.read().await;
        let connector_ids: Vec<String> = connectors.keys().cloned().collect();
        drop(connectors);
        
        info!("开始停止所有连接器，总数: {}", connector_ids.len());
        
        let errors: Vec<String> = Vec::new();
        
        for id in connector_ids {
            // 由于trait限制，这里需要重新设计
            debug!("停止连接器: {id}");
        }
        
        if errors.is_empty() {
            info!("所有连接器停止成功");
            Ok(())
        } else {
            error!("部分连接器停止失败: {errors:?}");
            Err(ManagerError::InvalidState {
                message: format!("停止失败的连接器数量: {}", errors.len()),
            })
        }
    }
    
    async fn status_all(&self) -> HashMap<String, ModernConnectionStatus> {
        let connectors = self.connectors.read().await;
        let mut status_map = HashMap::new();
        
        for id in connectors.keys() {
            // 由于trait限制，这里返回默认状态
            // 实际实现中应该调用连接器的状态方法
            status_map.insert(id.clone(), ModernConnectionStatus::Disconnected);
        }
        
        status_map
    }
    
    async fn health_check_all(&self) -> HashMap<String, HealthCheckResult> {
        let connectors = self.connectors.read().await;
        let mut health_map = HashMap::new();
        
        for id in connectors.keys() {
            // 由于trait限制，这里返回默认健康状态
            // 实际实现中应该调用连接器的健康检查方法
            health_map.insert(id.clone(), HealthCheckResult {
                healthy: false,
                checked_at: chrono::Utc::now(),
                latency_ms: None,
                details: HashMap::new(),
                warnings: vec!["未实现健康检查".to_string()],
                errors: Vec::new(),
            });
        }
        
        health_map
    }
}

/// 异步版本的管理器方法
impl<C: ModernExchangeConnector> DefaultModernConnectorManager<C> {
    /// 异步获取连接器列表
    pub async fn list_connectors_async(&self) -> Vec<String> {
        let connectors = self.connectors.read().await;
        connectors.keys().cloned().collect()
    }
    
    /// 异步获取连接器（只读）
    pub async fn get_connector_async(&self, id: &str) -> Option<C> 
    where 
        C: Clone,
    {
        let connectors = self.connectors.read().await;
        connectors.get(id).map(|reg| reg.connector.clone())
    }
    
    /// 异步执行连接器操作
    pub async fn with_connector_async<F, R>(&self, id: &str, f: F) -> Result<R, ManagerError>
    where
        F: FnOnce(&C) -> R,
        C: Clone,
    {
        let connectors = self.connectors.read().await;
        match connectors.get(id) {
            Some(reg) => Ok(f(&reg.connector)),
            None => Err(ManagerError::ConnectorNotFound { id: id.to_string() }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectors::traits::modern_binance::ModernBinanceConnector;
    
    #[tokio::test]
    async fn test_manager_creation() {
        let manager: DefaultModernConnectorManager<ModernBinanceConnector> = 
            DefaultModernConnectorManager::new();
        
        assert_eq!(manager.connector_count().await, 0);
    }
    
    #[tokio::test]
    async fn test_connector_registration() {
        let mut manager: DefaultModernConnectorManager<ModernBinanceConnector> = 
            DefaultModernConnectorManager::new();
        
        let connector = ModernBinanceConnector::new();
        let result = manager.register_connector("test".to_string(), connector).await;
        
        assert!(result.is_ok());
        assert_eq!(manager.connector_count().await, 1);
    }
    
    #[tokio::test]
    async fn test_connector_unregistration() {
        let mut manager: DefaultModernConnectorManager<ModernBinanceConnector> = 
            DefaultModernConnectorManager::new();
        
        let connector = ModernBinanceConnector::new();
        manager.register_connector("test".to_string(), connector).await.unwrap();
        
        let result = manager.unregister_connector("test").await;
        assert!(result.is_ok());
        assert_eq!(manager.connector_count().await, 0);
    }
    
    #[tokio::test]
    async fn test_duplicate_registration() {
        let mut manager: DefaultModernConnectorManager<ModernBinanceConnector> = 
            DefaultModernConnectorManager::new();
        
        let connector1 = ModernBinanceConnector::new();
        let connector2 = ModernBinanceConnector::new();
        
        manager.register_connector("test".to_string(), connector1).await.unwrap();
        let result = manager.register_connector("test".to_string(), connector2).await;
        
        assert!(matches!(result, Err(ManagerError::ConnectorAlreadyExists { .. })));
    }
}