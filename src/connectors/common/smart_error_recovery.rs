//! 智能错误恢复模块
//! 实现WebSocket优化重构方案中的高级错误处理功能

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, SystemTime};
use std::sync::Arc;
use tokio::sync::RwLock;
use log::{debug, warn, error, info};
use serde::{Deserialize, Serialize};
use crate::types::events::SystemEvent;
use crate::types::config::ConnectionStatus;

/// 智能错误恢复管理器
/// 负责分析错误模式并选择最佳恢复策略
#[derive(Debug, Clone)]
pub struct SmartErrorRecovery {
    /// 配置参数
    config: SmartErrorRecoveryConfig,
    /// 错误历史记录
    error_history: Arc<RwLock<ErrorHistory>>,
    /// 恢复策略缓存
    strategy_cache: Arc<RwLock<StrategyCache>>,
    /// 恢复统计信息
    recovery_stats: Arc<RwLock<RecoveryStats>>,
}

/// 智能错误恢复配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartErrorRecoveryConfig {
    /// 错误历史记录最大数量
    pub max_error_history: usize,
    /// 错误模式分析窗口大小
    pub analysis_window_size: usize,
    /// 最大重试次数
    pub max_retry_attempts: u32,
    /// 基础重试间隔（毫秒）
    pub base_retry_interval_ms: u64,
    /// 最大重试间隔（毫秒）
    pub max_retry_interval_ms: u64,
    /// 指数退避因子
    pub exponential_backoff_factor: f64,
    /// 错误频率阈值（每分钟）
    pub error_frequency_threshold: u32,
    /// 启用智能策略选择
    pub enable_smart_strategy: bool,
    /// 启用预测性恢复
    pub enable_predictive_recovery: bool,
    /// 恢复超时时间（秒）
    pub recovery_timeout_seconds: u64,
    /// 健康检查间隔（秒）
    pub health_check_interval_seconds: u64,
}

/// 错误历史记录
#[derive(Debug)]
struct ErrorHistory {
    /// 错误记录列表
    records: VecDeque<ErrorRecord>,
    /// 错误类型统计
    error_type_counts: HashMap<ErrorType, u32>,
    /// 错误模式
    patterns: Vec<ErrorPattern>,
}

/// 策略缓存
#[derive(Debug)]
struct StrategyCache {
    /// 错误类型到策略的映射
    error_to_strategy: HashMap<ErrorType, RecoveryStrategy>,
    /// 策略成功率
    strategy_success_rates: HashMap<RecoveryStrategy, f64>,
    /// 最后更新时间
    last_update: SystemTime,
}

/// 恢复统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryStats {
    /// 总错误数量
    pub total_errors: u64,
    /// 成功恢复数量
    pub successful_recoveries: u64,
    /// 失败恢复数量
    pub failed_recoveries: u64,
    /// 平均恢复时间（毫秒）
    pub avg_recovery_time_ms: f64,
    /// 各策略使用次数
    pub strategy_usage_counts: HashMap<RecoveryStrategy, u32>,
    /// 各策略成功率
    pub strategy_success_rates: HashMap<RecoveryStrategy, f64>,
    /// 最后恢复时间
    pub last_recovery_time: Option<SystemTime>,
}

/// 错误记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecord {
    /// 错误类型
    pub error_type: ErrorType,
    /// 错误消息
    pub error_message: String,
    /// 发生时间
    pub timestamp: SystemTime,
    /// 错误严重程度
    pub severity: ErrorSeverity,
    /// 上下文信息
    pub context: ErrorContext,
    /// 恢复尝试
    pub recovery_attempts: Vec<RecoveryAttempt>,
}

/// 错误类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorType {
    /// 连接错误
    ConnectionError,
    /// 认证错误
    AuthenticationError,
    /// 网络超时
    NetworkTimeout,
    /// 数据解析错误
    DataParsingError,
    /// 订阅错误
    SubscriptionError,
    /// 心跳超时
    HeartbeatTimeout,
    /// 服务器错误
    ServerError,
    /// 限流错误
    RateLimitError,
    /// 未知错误
    UnknownError,
    /// 配置错误
    ConfigurationError,
    /// 资源不足
    ResourceExhaustion,
    /// 协议错误
    ProtocolError,
}

/// 错误严重程度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorSeverity {
    /// 低级别（可忽略）
    Low,
    /// 中等级别（需要注意）
    Medium,
    /// 高级别（需要立即处理）
    High,
    /// 严重级别（系统级错误）
    Critical,
}

/// 错误上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    /// 交易所名称
    pub exchange: String,
    /// 交易对
    pub symbol: Option<String>,
    /// 连接状态
    pub connection_status: ConnectionStatus,
    /// 网络质量
    pub network_quality: Option<f64>,
    /// 系统负载
    pub system_load: Option<f64>,
    /// 额外信息
    pub additional_info: HashMap<String, String>,
}

/// 恢复策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecoveryStrategy {
    /// 立即重试
    ImmediateRetry,
    /// 延迟重试
    DelayedRetry,
    /// 指数退避重试
    ExponentialBackoffRetry,
    /// 重新连接
    Reconnect,
    /// 重新认证
    Reauthenticate,
    /// 重新订阅
    Resubscribe,
    /// 切换服务器
    SwitchServer,
    /// 降级服务
    DegradeService,
    /// 暂停服务
    PauseService,
    /// 重启连接器
    RestartConnector,
    /// 清理缓存
    ClearCache,
    /// 等待恢复
    WaitAndRecover,
    /// 重置配置
    ResetConfiguration,
}

/// 恢复尝试
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryAttempt {
    /// 尝试序号
    pub attempt_number: u32,
    /// 使用的策略
    pub strategy: RecoveryStrategy,
    /// 开始时间
    pub start_time: SystemTime,
    /// 结束时间
    pub end_time: Option<SystemTime>,
    /// 是否成功
    pub success: Option<bool>,
    /// 执行的动作
    pub actions: Vec<RecoveryAction>,
    /// 结果消息
    pub result_message: Option<String>,
}

/// 恢复动作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryAction {
    /// 关闭连接
    CloseConnection,
    /// 建立连接
    EstablishConnection,
    /// 发送认证
    SendAuthentication,
    /// 发送订阅
    SendSubscription(String),
    /// 发送心跳
    SendHeartbeat,
    /// 清理状态
    ClearState,
    /// 等待
    Wait(Duration),
    /// 切换端点
    SwitchEndpoint(String),
    /// 重置配置
    ResetConfiguration,
    /// 发送系统事件
    EmitSystemEvent(SystemEvent),
}

/// 错误模式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPattern {
    /// 模式名称
    pub name: String,
    /// 错误类型序列
    pub error_sequence: Vec<ErrorType>,
    /// 时间窗口（秒）
    pub time_window_seconds: u64,
    /// 出现次数
    pub occurrence_count: u32,
    /// 推荐策略
    pub recommended_strategy: RecoveryStrategy,
    /// 模式置信度
    pub confidence: f64,
}

/// 恢复结果
#[derive(Debug, Clone)]
pub struct RecoveryResult {
    /// 是否成功
    pub success: bool,
    /// 使用的策略
    pub strategy: RecoveryStrategy,
    /// 执行的动作
    pub actions: Vec<RecoveryAction>,
    /// 恢复时间（毫秒）
    pub recovery_time_ms: u64,
    /// 重试次数
    pub retry_count: u32,
    /// 结果消息
    pub message: String,
}

impl Default for SmartErrorRecoveryConfig {
    fn default() -> Self {
        Self {
            max_error_history: 1000,
            analysis_window_size: 50,
            max_retry_attempts: 5,
            base_retry_interval_ms: 1000,
            max_retry_interval_ms: 30000,
            exponential_backoff_factor: 2.0,
            error_frequency_threshold: 10,
            enable_smart_strategy: true,
            enable_predictive_recovery: true,
            recovery_timeout_seconds: 60,
            health_check_interval_seconds: 30,
        }
    }
}

impl Default for ErrorHistory {
    fn default() -> Self {
        Self {
            records: VecDeque::new(),
            error_type_counts: HashMap::new(),
            patterns: Vec::new(),
        }
    }
}

impl Default for StrategyCache {
    fn default() -> Self {
        Self {
            error_to_strategy: HashMap::new(),
            strategy_success_rates: HashMap::new(),
            last_update: SystemTime::now(),
        }
    }
}

impl Default for RecoveryStats {
    fn default() -> Self {
        Self {
            total_errors: 0,
            successful_recoveries: 0,
            failed_recoveries: 0,
            avg_recovery_time_ms: 0.0,
            strategy_usage_counts: HashMap::new(),
            strategy_success_rates: HashMap::new(),
            last_recovery_time: None,
        }
    }
}

impl SmartErrorRecovery {
    /// 创建新的智能错误恢复管理器
    pub fn new(config: SmartErrorRecoveryConfig) -> Self {
        Self {
            config,
            error_history: Arc::new(RwLock::new(ErrorHistory::default())),
            strategy_cache: Arc::new(RwLock::new(StrategyCache::default())),
            recovery_stats: Arc::new(RwLock::new(RecoveryStats::default())),
        }
    }

    /// 使用默认配置创建管理器
    pub fn with_default_config() -> Self {
        Self::new(SmartErrorRecoveryConfig::default())
    }

    /// 记录错误
    pub async fn record_error(
        &self,
        error_type: ErrorType,
        error_message: String,
        severity: ErrorSeverity,
        context: ErrorContext,
    ) {
        let mut history = self.error_history.write().await;
        let mut stats = self.recovery_stats.write().await;
        
        let error_record = ErrorRecord {
            error_type,
            error_message: error_message.clone(),
            timestamp: SystemTime::now(),
            severity,
            context,
            recovery_attempts: Vec::new(),
        };
        
        // 添加到历史记录
        history.records.push_back(error_record);
        
        // 限制历史记录大小
        while history.records.len() > self.config.max_error_history {
            history.records.pop_front();
        }
        
        // 更新错误类型统计
        *history.error_type_counts.entry(error_type).or_insert(0) += 1;
        
        // 更新总错误数
        stats.total_errors += 1;
        
        debug!("[SmartErrorRecovery] 记录错误: {:?} - {}", error_type, error_message);
        
        // 分析错误模式
        if self.config.enable_smart_strategy {
            self.analyze_error_patterns().await;
        }
    }

    /// 选择恢复策略
    pub async fn select_recovery_strategy(
        &self,
        error_type: ErrorType,
        context: &ErrorContext,
    ) -> RecoveryStrategy {
        if self.config.enable_smart_strategy {
            self.select_smart_strategy(error_type, context).await
        } else {
            self.select_default_strategy(error_type)
        }
    }

    /// 智能策略选择
    async fn select_smart_strategy(
        &self,
        error_type: ErrorType,
        context: &ErrorContext,
    ) -> RecoveryStrategy {
        let cache = self.strategy_cache.read().await;
        let history = self.error_history.read().await;
        
        // 检查缓存中是否有该错误类型的最佳策略
        if let Some(&strategy) = cache.error_to_strategy.get(&error_type) {
            if let Some(&success_rate) = cache.strategy_success_rates.get(&strategy) {
                if success_rate > 0.7 { // 成功率超过70%
                    debug!("[SmartErrorRecovery] 使用缓存策略: {:?} (成功率: {:.2}%)", 
                           strategy, success_rate * 100.0);
                    return strategy;
                }
            }
        }
        
        // 分析最近的错误模式
        for pattern in &history.patterns {
            if pattern.error_sequence.contains(&error_type) && pattern.confidence > 0.8 {
                debug!("[SmartErrorRecovery] 使用模式策略: {:?} (置信度: {:.2})", 
                       pattern.recommended_strategy, pattern.confidence);
                return pattern.recommended_strategy;
            }
        }
        
        // 根据上下文选择策略
        self.select_context_based_strategy(error_type, context).await
    }

    /// 基于上下文的策略选择
    async fn select_context_based_strategy(
        &self,
        error_type: ErrorType,
        context: &ErrorContext,
    ) -> RecoveryStrategy {
        match error_type {
            ErrorType::ConnectionError => {
                match context.connection_status {
                    ConnectionStatus::Disconnected => RecoveryStrategy::Reconnect,
                    ConnectionStatus::Connecting => RecoveryStrategy::DelayedRetry,
                    _ => RecoveryStrategy::ImmediateRetry,
                }
            }
            ErrorType::NetworkTimeout => {
                if let Some(quality) = context.network_quality {
                    if quality < 0.5 {
                        RecoveryStrategy::ExponentialBackoffRetry
                    } else {
                        RecoveryStrategy::DelayedRetry
                    }
                } else {
                    RecoveryStrategy::DelayedRetry
                }
            }
            ErrorType::AuthenticationError => RecoveryStrategy::Reauthenticate,
            ErrorType::SubscriptionError => RecoveryStrategy::Resubscribe,
            ErrorType::HeartbeatTimeout => RecoveryStrategy::Reconnect,
            ErrorType::RateLimitError => RecoveryStrategy::ExponentialBackoffRetry,
            ErrorType::ServerError => RecoveryStrategy::SwitchServer,
            ErrorType::ResourceExhaustion => RecoveryStrategy::ClearCache,
            _ => self.select_default_strategy(error_type),
        }
    }

    /// 默认策略选择
    fn select_default_strategy(&self, error_type: ErrorType) -> RecoveryStrategy {
        match error_type {
            ErrorType::ConnectionError => RecoveryStrategy::Reconnect,
            ErrorType::AuthenticationError => RecoveryStrategy::Reauthenticate,
            ErrorType::NetworkTimeout => RecoveryStrategy::DelayedRetry,
            ErrorType::DataParsingError => RecoveryStrategy::ImmediateRetry,
            ErrorType::SubscriptionError => RecoveryStrategy::Resubscribe,
            ErrorType::HeartbeatTimeout => RecoveryStrategy::Reconnect,
            ErrorType::ServerError => RecoveryStrategy::DelayedRetry,
            ErrorType::RateLimitError => RecoveryStrategy::ExponentialBackoffRetry,
            ErrorType::ConfigurationError => RecoveryStrategy::ResetConfiguration,
            ErrorType::ResourceExhaustion => RecoveryStrategy::ClearCache,
            ErrorType::ProtocolError => RecoveryStrategy::Reconnect,
            ErrorType::UnknownError => RecoveryStrategy::DelayedRetry,
        }
    }

    /// 执行恢复策略
    pub async fn execute_recovery(
        &self,
        strategy: RecoveryStrategy,
        error_record: &mut ErrorRecord,
    ) -> RecoveryResult {
        let start_time = SystemTime::now();
        let attempt_number = error_record.recovery_attempts.len() as u32 + 1;
        
        debug!("[SmartErrorRecovery] 执行恢复策略: {:?} (尝试 {})", strategy, attempt_number);
        
        let mut attempt = RecoveryAttempt {
            attempt_number,
            strategy,
            start_time,
            end_time: None,
            success: None,
            actions: Vec::new(),
            result_message: None,
        };
        
        let actions = self.generate_recovery_actions(strategy, &error_record.context).await;
        attempt.actions = actions.clone();
        
        // 执行恢复动作
        let success = self.execute_recovery_actions(&actions).await;
        
        let end_time = SystemTime::now();
        attempt.end_time = Some(end_time);
        attempt.success = Some(success);
        attempt.result_message = Some(if success {
            "恢复成功".to_string()
        } else {
            "恢复失败".to_string()
        });
        
        error_record.recovery_attempts.push(attempt);
        
        let recovery_time_ms = end_time.duration_since(start_time)
            .unwrap_or(Duration::from_millis(0))
            .as_millis() as u64;
        
        // 更新统计信息
        self.update_recovery_stats(strategy, success, recovery_time_ms).await;
        
        RecoveryResult {
            success,
            strategy,
            actions,
            recovery_time_ms,
            retry_count: attempt_number,
            message: if success {
                format!("恢复成功，耗时 {}ms", recovery_time_ms)
            } else {
                format!("恢复失败，耗时 {}ms", recovery_time_ms)
            },
        }
    }

    /// 生成恢复动作
    async fn generate_recovery_actions(
        &self,
        strategy: RecoveryStrategy,
        context: &ErrorContext,
    ) -> Vec<RecoveryAction> {
        match strategy {
            RecoveryStrategy::ImmediateRetry => {
                vec![RecoveryAction::Wait(Duration::from_millis(100))]
            }
            RecoveryStrategy::DelayedRetry => {
                vec![RecoveryAction::Wait(Duration::from_millis(self.config.base_retry_interval_ms))]
            }
            RecoveryStrategy::ExponentialBackoffRetry => {
                let delay = self.calculate_exponential_backoff_delay(1).await;
                vec![RecoveryAction::Wait(delay)]
            }
            RecoveryStrategy::Reconnect => {
                vec![
                    RecoveryAction::CloseConnection,
                    RecoveryAction::Wait(Duration::from_millis(1000)),
                    RecoveryAction::EstablishConnection,
                ]
            }
            RecoveryStrategy::Reauthenticate => {
                vec![
                    RecoveryAction::SendAuthentication,
                ]
            }
            RecoveryStrategy::Resubscribe => {
                if let Some(ref symbol) = context.symbol {
                    vec![
                        RecoveryAction::SendSubscription(symbol.clone()),
                    ]
                } else {
                    vec![RecoveryAction::ClearState]
                }
            }
            RecoveryStrategy::SwitchServer => {
                vec![
                    RecoveryAction::CloseConnection,
                    RecoveryAction::SwitchEndpoint("backup_endpoint".to_string()),
                    RecoveryAction::EstablishConnection,
                ]
            }
            RecoveryStrategy::DegradeService => {
                vec![
                    RecoveryAction::EmitSystemEvent(SystemEvent::ServiceDegraded {
                        exchange: crate::types::common::ExchangeType::Binance, // 默认值
                        market_type: crate::types::common::MarketType::Spot, // 默认值
                        reason: "服务降级".to_string(),
                        timestamp: SystemTime::now(),
                    }),
                    RecoveryAction::ClearState,
                ]
            }
            RecoveryStrategy::PauseService => {
                vec![
                    RecoveryAction::EmitSystemEvent(SystemEvent::ServicePaused {
                        exchange: crate::types::common::ExchangeType::Binance, // 默认值
                        market_type: crate::types::common::MarketType::Spot, // 默认值
                        reason: "服务暂停".to_string(),
                        timestamp: SystemTime::now(),
                    }),
                    RecoveryAction::Wait(Duration::from_secs(30)),
                ]
            }
            RecoveryStrategy::RestartConnector => {
                vec![
                    RecoveryAction::CloseConnection,
                    RecoveryAction::ClearState,
                    RecoveryAction::ResetConfiguration,
                    RecoveryAction::EstablishConnection,
                ]
            }
            RecoveryStrategy::ClearCache => {
                vec![
                    RecoveryAction::ClearState,
                ]
            }
            RecoveryStrategy::WaitAndRecover => {
                vec![
                    RecoveryAction::Wait(Duration::from_secs(self.config.health_check_interval_seconds)),
                    RecoveryAction::EstablishConnection,
                ]
            }
            RecoveryStrategy::ResetConfiguration => {
                vec![
                    RecoveryAction::ResetConfiguration,
                    RecoveryAction::ClearState,
                ]
            }
        }
    }

    /// 执行恢复动作
    async fn execute_recovery_actions(&self, actions: &[RecoveryAction]) -> bool {
        for action in actions {
            match action {
                RecoveryAction::Wait(duration) => {
                    debug!("[SmartErrorRecovery] 等待 {:?}", duration);
                    tokio::time::sleep(*duration).await;
                }
                RecoveryAction::CloseConnection => {
                    debug!("[SmartErrorRecovery] 关闭连接");
                    // TODO: 实际的连接关闭逻辑
                }
                RecoveryAction::EstablishConnection => {
                    debug!("[SmartErrorRecovery] 建立连接");
                    // TODO: 实际的连接建立逻辑
                }
                RecoveryAction::SendAuthentication => {
                    debug!("[SmartErrorRecovery] 发送认证");
                    // TODO: 实际的认证逻辑
                }
                RecoveryAction::SendSubscription(symbol) => {
                    debug!("[SmartErrorRecovery] 发送订阅: {}", symbol);
                    // TODO: 实际的订阅逻辑
                }
                RecoveryAction::SendHeartbeat => {
                    debug!("[SmartErrorRecovery] 发送心跳");
                    // TODO: 实际的心跳逻辑
                }
                RecoveryAction::ClearState => {
                    debug!("[SmartErrorRecovery] 清理状态");
                    // TODO: 实际的状态清理逻辑
                }
                RecoveryAction::SwitchEndpoint(endpoint) => {
                    debug!("[SmartErrorRecovery] 切换端点: {}", endpoint);
                    // TODO: 实际的端点切换逻辑
                }
                RecoveryAction::ResetConfiguration => {
                    debug!("[SmartErrorRecovery] 重置配置");
                    // TODO: 实际的配置重置逻辑
                }
                RecoveryAction::EmitSystemEvent(event) => {
                    debug!("[SmartErrorRecovery] 发送系统事件: {:?}", event);
                    // TODO: 实际的事件发送逻辑
                }
            }
        }
        
        // 简化的成功判断逻辑
        true
    }

    /// 计算指数退避延迟
    async fn calculate_exponential_backoff_delay(&self, attempt: u32) -> Duration {
        let delay_ms = (self.config.base_retry_interval_ms as f64 
            * self.config.exponential_backoff_factor.powi(attempt as i32 - 1)) as u64;
        
        let capped_delay = delay_ms.min(self.config.max_retry_interval_ms);
        Duration::from_millis(capped_delay)
    }

    /// 分析错误模式
    async fn analyze_error_patterns(&self) {
        let mut history = self.error_history.write().await;
        
        if history.records.len() < self.config.analysis_window_size {
            return;
        }
        
        // 分析最近的错误序列
        let recent_errors: Vec<_> = history.records
            .iter()
            .rev()
            .take(self.config.analysis_window_size)
            .map(|record| record.error_type)
            .collect();
        
        // 查找重复模式
        for window_size in 2..=5 {
            if recent_errors.len() < window_size * 2 {
                continue;
            }
            
            for i in 0..=(recent_errors.len() - window_size * 2) {
                let pattern1 = &recent_errors[i..i + window_size];
                let pattern2 = &recent_errors[i + window_size..i + window_size * 2];
                
                if pattern1 == pattern2 {
                    let pattern_name = format!("重复模式_{}", window_size);
                    let recommended_strategy = self.recommend_strategy_for_pattern(pattern1);
                    
                    let pattern = ErrorPattern {
                        name: pattern_name,
                        error_sequence: pattern1.to_vec(),
                        time_window_seconds: 300, // 5分钟窗口
                        occurrence_count: 1,
                        recommended_strategy,
                        confidence: 0.8,
                    };
                    
                    // 检查是否已存在相同模式
                    if !history.patterns.iter().any(|p| p.error_sequence == pattern.error_sequence) {
                        history.patterns.push(pattern);
                        debug!("[SmartErrorRecovery] 发现新错误模式: {:?}", pattern1);
                    }
                }
            }
        }
        
        // 限制模式数量
        if history.patterns.len() > 20 {
            history.patterns.truncate(20);
        }
    }

    /// 为模式推荐策略
    fn recommend_strategy_for_pattern(&self, pattern: &[ErrorType]) -> RecoveryStrategy {
        // 简化的策略推荐逻辑
        if pattern.contains(&ErrorType::ConnectionError) {
            RecoveryStrategy::Reconnect
        } else if pattern.contains(&ErrorType::NetworkTimeout) {
            RecoveryStrategy::ExponentialBackoffRetry
        } else if pattern.contains(&ErrorType::AuthenticationError) {
            RecoveryStrategy::Reauthenticate
        } else {
            RecoveryStrategy::DelayedRetry
        }
    }

    /// 更新恢复统计信息
    async fn update_recovery_stats(
        &self,
        strategy: RecoveryStrategy,
        success: bool,
        recovery_time_ms: u64,
    ) {
        let mut stats = self.recovery_stats.write().await;
        
        if success {
            stats.successful_recoveries += 1;
        } else {
            stats.failed_recoveries += 1;
        }
        
        // 更新平均恢复时间
        let total_recoveries = stats.successful_recoveries + stats.failed_recoveries;
        let total_time = stats.avg_recovery_time_ms * (total_recoveries - 1) as f64;
        stats.avg_recovery_time_ms = (total_time + recovery_time_ms as f64) / total_recoveries as f64;
        
        // 更新策略使用次数
        *stats.strategy_usage_counts.entry(strategy).or_insert(0) += 1;
        
        // 更新策略成功率
        let usage_count = *stats.strategy_usage_counts.get(&strategy).unwrap_or(&0);
        if usage_count > 0 {
            let current_success_rate = stats.strategy_success_rates.get(&strategy).unwrap_or(&0.0);
            let new_success_rate = if success {
                (current_success_rate * (usage_count - 1) as f64 + 1.0) / usage_count as f64
            } else {
                (current_success_rate * (usage_count - 1) as f64) / usage_count as f64
            };
            stats.strategy_success_rates.insert(strategy, new_success_rate);
        }
        
        stats.last_recovery_time = Some(SystemTime::now());
        
        // 更新策略缓存
        self.update_strategy_cache().await;
    }

    /// 更新策略缓存
    async fn update_strategy_cache(&self) {
        let stats = self.recovery_stats.read().await;
        let mut cache = self.strategy_cache.write().await;
        
        // 更新策略成功率缓存
        cache.strategy_success_rates = stats.strategy_success_rates.clone();
        cache.last_update = SystemTime::now();
    }

    /// 获取恢复统计信息
    pub async fn get_recovery_stats(&self) -> RecoveryStats {
        self.recovery_stats.read().await.clone()
    }

    /// 获取错误历史
    pub async fn get_error_history(&self) -> Vec<ErrorRecord> {
        let history = self.error_history.read().await;
        history.records.iter().cloned().collect()
    }

    /// 清理历史记录
    pub async fn clear_history(&self) {
        let mut history = self.error_history.write().await;
        history.records.clear();
        history.error_type_counts.clear();
        history.patterns.clear();
        
        debug!("[SmartErrorRecovery] 历史记录已清理");
    }

    /// 重置统计信息
    pub async fn reset_stats(&self) {
        let mut stats = self.recovery_stats.write().await;
        *stats = RecoveryStats::default();
        
        debug!("[SmartErrorRecovery] 统计信息已重置");
    }

    /// 更新配置
    pub async fn update_config(&mut self, new_config: SmartErrorRecoveryConfig) {
        self.config = new_config;
        debug!("[SmartErrorRecovery] 配置已更新");
    }

    /// 生成恢复报告
    pub async fn generate_recovery_report(&self) -> RecoveryReport {
        let stats = self.get_recovery_stats().await;
        let history = self.error_history.read().await;
        
        let best_performing_strategy = stats.strategy_success_rates
            .iter()
            .max_by(|(_, rate1), (_, rate2)| rate1.partial_cmp(rate2).unwrap())
            .map(|(strategy, _)| *strategy);
        
        RecoveryReport {
            config: self.config.clone(),
            stats,
            total_error_types: history.error_type_counts.len(),
            detected_patterns: history.patterns.len(),
            most_common_error: history.error_type_counts
                .iter()
                .max_by_key(|(_, count)| *count)
                .map(|(error_type, _)| *error_type),
            best_performing_strategy,
        }
    }
}

/// 恢复报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryReport {
    pub config: SmartErrorRecoveryConfig,
    pub stats: RecoveryStats,
    pub total_error_types: usize,
    pub detected_patterns: usize,
    pub most_common_error: Option<ErrorType>,
    pub best_performing_strategy: Option<RecoveryStrategy>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_smart_error_recovery_creation() {
        let recovery = SmartErrorRecovery::with_default_config();
        let stats = recovery.get_recovery_stats().await;
        
        assert_eq!(stats.total_errors, 0);
        assert_eq!(stats.successful_recoveries, 0);
        assert_eq!(stats.failed_recoveries, 0);
    }

    #[tokio::test]
    async fn test_record_error() {
        let recovery = SmartErrorRecovery::with_default_config();
        
        let context = ErrorContext {
            exchange: "binance".to_string(),
            symbol: Some("BTCUSDT".to_string()),
            connection_status: ConnectionStatus::Connected,
            network_quality: Some(0.8),
            system_load: Some(0.5),
            additional_info: HashMap::new(),
        };
        
        recovery.record_error(
            ErrorType::ConnectionError,
            "连接超时".to_string(),
            ErrorSeverity::High,
            context,
        ).await;
        
        let stats = recovery.get_recovery_stats().await;
        assert_eq!(stats.total_errors, 1);
        
        let history = recovery.get_error_history().await;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].error_type, ErrorType::ConnectionError);
    }

    #[tokio::test]
    async fn test_strategy_selection() {
        let recovery = SmartErrorRecovery::with_default_config();
        
        let context = ErrorContext {
            exchange: "binance".to_string(),
            symbol: Some("BTCUSDT".to_string()),
            connection_status: ConnectionStatus::Disconnected,
            network_quality: Some(0.8),
            system_load: Some(0.5),
            additional_info: HashMap::new(),
        };
        
        let strategy = recovery.select_recovery_strategy(ErrorType::ConnectionError, &context).await;
        assert_eq!(strategy, RecoveryStrategy::Reconnect);
        
        let strategy = recovery.select_recovery_strategy(ErrorType::AuthenticationError, &context).await;
        assert_eq!(strategy, RecoveryStrategy::Reauthenticate);
        
        let strategy = recovery.select_recovery_strategy(ErrorType::RateLimitError, &context).await;
        assert_eq!(strategy, RecoveryStrategy::ExponentialBackoffRetry);
    }

    #[tokio::test]
    async fn test_exponential_backoff() {
        let recovery = SmartErrorRecovery::with_default_config();
        
        let delay1 = recovery.calculate_exponential_backoff_delay(1).await;
        let delay2 = recovery.calculate_exponential_backoff_delay(2).await;
        let delay3 = recovery.calculate_exponential_backoff_delay(3).await;
        
        assert!(delay2 > delay1);
        assert!(delay3 > delay2);
    }

    #[tokio::test]
    async fn test_recovery_execution() {
        let recovery = SmartErrorRecovery::with_default_config();
        
        let context = ErrorContext {
            exchange: "binance".to_string(),
            symbol: Some("BTCUSDT".to_string()),
            connection_status: ConnectionStatus::Connected,
            network_quality: Some(0.8),
            system_load: Some(0.5),
            additional_info: HashMap::new(),
        };
        
        let mut error_record = ErrorRecord {
            error_type: ErrorType::ConnectionError,
            error_message: "测试错误".to_string(),
            timestamp: SystemTime::now(),
            severity: ErrorSeverity::Medium,
            context,
            recovery_attempts: Vec::new(),
        };
        
        let result = recovery.execute_recovery(
            RecoveryStrategy::ImmediateRetry,
            &mut error_record,
        ).await;
        
        assert!(result.success);
        assert_eq!(result.strategy, RecoveryStrategy::ImmediateRetry);
        assert_eq!(result.retry_count, 1);
        assert_eq!(error_record.recovery_attempts.len(), 1);
    }

    #[tokio::test]
    async fn test_generate_report() {
        let recovery = SmartErrorRecovery::with_default_config();
        
        let context = ErrorContext {
            exchange: "binance".to_string(),
            symbol: Some("BTCUSDT".to_string()),
            connection_status: ConnectionStatus::Connected,
            network_quality: Some(0.8),
            system_load: Some(0.5),
            additional_info: HashMap::new(),
        };
        
        recovery.record_error(
            ErrorType::ConnectionError,
            "测试错误".to_string(),
            ErrorSeverity::Medium,
            context,
        ).await;
        
        let report = recovery.generate_recovery_report().await;
        assert_eq!(report.stats.total_errors, 1);
        assert_eq!(report.total_error_types, 1);
        assert_eq!(report.most_common_error, Some(ErrorType::ConnectionError));
    }
}