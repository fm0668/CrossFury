//! 紧急Ping管理器
//! 实现WebSocket优化重构方案中的紧急Ping功能

use std::time::{Duration, SystemTime};
use std::sync::Arc;
use tokio::sync::RwLock;
use log::{debug, warn, error};
use serde::{Deserialize, Serialize};

/// 紧急Ping管理器
/// 负责在网络质量下降时发送紧急ping来维持连接
#[derive(Debug, Clone)]
pub struct EmergencyPingManager {
    /// 配置参数
    config: EmergencyPingConfig,
    /// 当前状态
    state: Arc<RwLock<EmergencyPingState>>,
}

/// 紧急Ping配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyPingConfig {
    /// 触发紧急ping的延迟阈值（毫秒）
    pub trigger_threshold_ms: u64,
    /// 紧急ping超时时间（毫秒）
    pub timeout_ms: u64,
    /// 最大重试次数
    pub max_retries: u32,
    /// 重试间隔（毫秒）
    pub retry_interval_ms: u64,
    /// 是否启用紧急ping
    pub enabled: bool,
}

/// 紧急Ping状态
#[derive(Debug, Clone)]
pub struct EmergencyPingState {
    /// 最后一次ping时间
    last_ping_time: Option<SystemTime>,
    /// 最后一次pong时间
    last_pong_time: Option<SystemTime>,
    /// 当前重试次数
    retry_count: u32,
    /// 是否正在进行紧急ping
    is_emergency_pinging: bool,
    /// 连续失败次数
    consecutive_failures: u32,
}

/// 紧急Ping结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmergencyPingResult {
    /// 成功，返回延迟时间
    Success(Duration),
    /// 超时
    Timeout,
    /// 失败，包含错误信息
    Failed(String),
    /// 达到最大重试次数
    MaxRetriesExceeded,
}

impl Default for EmergencyPingConfig {
    fn default() -> Self {
        Self {
            trigger_threshold_ms: 2000, // 2秒
            timeout_ms: 5000,           // 5秒
            max_retries: 3,
            retry_interval_ms: 1000,     // 1秒
            enabled: true,
        }
    }
}

impl Default for EmergencyPingState {
    fn default() -> Self {
        Self {
            last_ping_time: None,
            last_pong_time: None,
            retry_count: 0,
            is_emergency_pinging: false,
            consecutive_failures: 0,
        }
    }
}

impl EmergencyPingManager {
    /// 创建新的紧急Ping管理器
    pub fn new(config: EmergencyPingConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(EmergencyPingState::default())),
        }
    }

    /// 使用默认配置创建管理器
    pub fn with_default_config() -> Self {
        Self::new(EmergencyPingConfig::default())
    }

    /// 检查是否需要发送紧急ping
    pub async fn should_send_emergency_ping(&self) -> bool {
        if !self.config.enabled {
            return false;
        }

        let state = self.state.read().await;
        
        // 如果已经在进行紧急ping，不重复发送
        if state.is_emergency_pinging {
            return false;
        }

        // 检查是否有最后一次pong时间
        if let Some(last_pong) = state.last_pong_time {
            let elapsed = last_pong.elapsed().unwrap_or(Duration::from_secs(0));
            let threshold = Duration::from_millis(self.config.trigger_threshold_ms);
            
            if elapsed > threshold {
                debug!("[EmergencyPing] 触发紧急ping: 距离上次pong已过 {}ms", elapsed.as_millis());
                return true;
            }
        } else {
            // 如果从未收到过pong，也需要发送紧急ping
            debug!("[EmergencyPing] 触发紧急ping: 从未收到pong响应");
            return true;
        }

        false
    }

    /// 开始紧急ping流程
    pub async fn start_emergency_ping(&self) -> Result<(), String> {
        let mut state = self.state.write().await;
        
        if state.is_emergency_pinging {
            return Err("紧急ping已在进行中".to_string());
        }

        if state.retry_count >= self.config.max_retries {
            return Err("已达到最大重试次数".to_string());
        }

        state.is_emergency_pinging = true;
        state.last_ping_time = Some(SystemTime::now());
        state.retry_count += 1;

        debug!("[EmergencyPing] 开始紧急ping (第{}次尝试)", state.retry_count);
        Ok(())
    }

    /// 处理紧急ping响应
    pub async fn handle_emergency_pong(&self) -> EmergencyPingResult {
        let mut state = self.state.write().await;
        
        if !state.is_emergency_pinging {
            warn!("[EmergencyPing] 收到pong但未在进行紧急ping");
            return EmergencyPingResult::Failed("未在进行紧急ping".to_string());
        }

        let now = SystemTime::now();
        state.last_pong_time = Some(now);
        state.is_emergency_pinging = false;
        
        if let Some(ping_time) = state.last_ping_time {
            let latency = now.duration_since(ping_time).unwrap_or(Duration::from_secs(0));
            
            // 重置失败计数
            state.retry_count = 0;
            state.consecutive_failures = 0;
            
            debug!("[EmergencyPing] 紧急ping成功，延迟: {}ms", latency.as_millis());
            EmergencyPingResult::Success(latency)
        } else {
            error!("[EmergencyPing] 内部错误：ping时间未记录");
            EmergencyPingResult::Failed("ping时间未记录".to_string())
        }
    }

    /// 处理紧急ping超时
    pub async fn handle_emergency_timeout(&self) -> EmergencyPingResult {
        let mut state = self.state.write().await;
        
        state.is_emergency_pinging = false;
        state.consecutive_failures += 1;
        
        if state.retry_count >= self.config.max_retries {
            warn!("[EmergencyPing] 紧急ping达到最大重试次数: {}", self.config.max_retries);
            
            // 重置状态以便下次重新开始
            state.retry_count = 0;
            
            EmergencyPingResult::MaxRetriesExceeded
        } else {
            warn!("[EmergencyPing] 紧急ping超时 (第{}次)", state.retry_count);
            EmergencyPingResult::Timeout
        }
    }

    /// 重置紧急ping状态
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        *state = EmergencyPingState::default();
        debug!("[EmergencyPing] 状态已重置");
    }

    /// 获取当前状态信息
    pub async fn get_status(&self) -> EmergencyPingStatus {
        let state = self.state.read().await;
        
        EmergencyPingStatus {
            is_emergency_pinging: state.is_emergency_pinging,
            retry_count: state.retry_count,
            consecutive_failures: state.consecutive_failures,
            last_ping_time: state.last_ping_time,
            last_pong_time: state.last_pong_time,
            config: self.config.clone(),
        }
    }

    /// 更新配置
    pub async fn update_config(&mut self, new_config: EmergencyPingConfig) {
        self.config = new_config;
        debug!("[EmergencyPing] 配置已更新");
    }

    /// 获取建议的下次ping时间
    pub async fn get_next_ping_time(&self) -> Option<SystemTime> {
        let state = self.state.read().await;
        
        if state.is_emergency_pinging {
            return None;
        }

        if let Some(last_pong) = state.last_pong_time {
            let next_check = last_pong + Duration::from_millis(self.config.trigger_threshold_ms);
            Some(next_check)
        } else {
            // 如果从未收到pong，立即检查
            Some(SystemTime::now())
        }
    }
}

/// 紧急Ping状态信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyPingStatus {
    pub is_emergency_pinging: bool,
    pub retry_count: u32,
    pub consecutive_failures: u32,
    pub last_ping_time: Option<SystemTime>,
    pub last_pong_time: Option<SystemTime>,
    pub config: EmergencyPingConfig,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration as TokioDuration};

    #[tokio::test]
    async fn test_emergency_ping_manager_creation() {
        let manager = EmergencyPingManager::with_default_config();
        let status = manager.get_status().await;
        
        assert!(!status.is_emergency_pinging);
        assert_eq!(status.retry_count, 0);
        assert_eq!(status.consecutive_failures, 0);
    }

    #[tokio::test]
    async fn test_should_send_emergency_ping() {
        let config = EmergencyPingConfig {
            trigger_threshold_ms: 100, // 100ms for testing
            ..Default::default()
        };
        let manager = EmergencyPingManager::new(config);
        
        // 初始状态不应该发送紧急ping
        assert!(manager.should_send_emergency_ping().await);
        
        // 模拟收到pong
        {
            let mut state = manager.state.write().await;
            state.last_pong_time = Some(SystemTime::now());
        }
        
        // 刚收到pong，不应该发送紧急ping
        assert!(!manager.should_send_emergency_ping().await);
        
        // 等待超过阈值时间
        sleep(TokioDuration::from_millis(150)).await;
        
        // 现在应该发送紧急ping
        assert!(manager.should_send_emergency_ping().await);
    }

    #[tokio::test]
    async fn test_emergency_ping_flow() {
        let manager = EmergencyPingManager::with_default_config();
        
        // 开始紧急ping
        assert!(manager.start_emergency_ping().await.is_ok());
        
        let status = manager.get_status().await;
        assert!(status.is_emergency_pinging);
        assert_eq!(status.retry_count, 1);
        
        // 处理成功的pong
        let result = manager.handle_emergency_pong().await;
        assert!(matches!(result, EmergencyPingResult::Success(_)));
        
        let status = manager.get_status().await;
        assert!(!status.is_emergency_pinging);
        assert_eq!(status.retry_count, 0);
    }

    #[tokio::test]
    async fn test_emergency_ping_timeout() {
        let manager = EmergencyPingManager::with_default_config();
        
        // 开始紧急ping
        assert!(manager.start_emergency_ping().await.is_ok());
        
        // 处理超时
        let result = manager.handle_emergency_timeout().await;
        assert_eq!(result, EmergencyPingResult::Timeout);
        
        let status = manager.get_status().await;
        assert!(!status.is_emergency_pinging);
        assert_eq!(status.consecutive_failures, 1);
    }

    #[tokio::test]
    async fn test_max_retries() {
        let config = EmergencyPingConfig {
            max_retries: 2,
            ..Default::default()
        };
        let manager = EmergencyPingManager::new(config);
        
        // 第一次重试
        assert!(manager.start_emergency_ping().await.is_ok());
        assert_eq!(manager.handle_emergency_timeout().await, EmergencyPingResult::Timeout);
        
        // 第二次重试
        assert!(manager.start_emergency_ping().await.is_ok());
        assert_eq!(manager.handle_emergency_timeout().await, EmergencyPingResult::MaxRetriesExceeded);
        
        // 应该可以重新开始
        assert!(manager.start_emergency_ping().await.is_ok());
    }
}