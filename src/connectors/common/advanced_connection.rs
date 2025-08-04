// src/connectors/common/advanced_connection.rs - 高级连接管理功能

use std::sync::{Arc, atomic::{AtomicUsize, AtomicBool, Ordering}};
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex};
use tokio_tungstenite::{WebSocketStream, MaybeTlsStream};
use tokio_tungstenite::tungstenite::Message;
use tokio::net::TcpStream;
use futures_util::sink::SinkExt;
use futures_util::stream::SplitSink;
use log::{info, warn, error, debug};
use crate::types::{ConnectorError, ConnectionQuality};
use chrono;

/// 紧急Ping管理器
/// 实现小交易所的紧急ping策略，在首次超时时立即发送ping
#[derive(Debug, Clone)]
pub struct EmergencyPingManager {
    last_ping_time: Arc<RwLock<Instant>>,
    consecutive_timeouts: Arc<AtomicUsize>,
    emergency_ping_active: Arc<AtomicBool>,
    threshold: u32,
}

impl EmergencyPingManager {
    pub fn new(threshold: u32) -> Self {
        Self {
            last_ping_time: Arc::new(RwLock::new(Instant::now())),
            consecutive_timeouts: Arc::new(AtomicUsize::new(0)),
            emergency_ping_active: Arc::new(AtomicBool::new(false)),
            threshold,
        }
    }
    
    /// 处理超时事件，决定是否触发紧急ping
    pub async fn handle_timeout(
        &self, 
        ws_sink: &Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>
    ) -> Result<bool, ConnectorError> {
        let timeout_count = self.consecutive_timeouts.fetch_add(1, Ordering::SeqCst);
        
        debug!("处理超时事件，连续超时次数: {}", timeout_count + 1);
        
        // 首次超时且未激活紧急ping时，立即触发
        if timeout_count == 0 && !self.emergency_ping_active.load(Ordering::SeqCst) {
            return self.execute_emergency_ping(ws_sink).await;
        }
        
        // 达到阈值时触发紧急ping
        if timeout_count + 1 >= self.threshold as usize {
            return self.execute_emergency_ping(ws_sink).await;
        }
        
        Ok(false)
    }
    
    /// 执行紧急ping
    pub async fn execute_emergency_ping(
        &self,
        ws_sink: &Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
    ) -> Result<bool, ConnectorError> {
        if self.emergency_ping_active.load(Ordering::SeqCst) {
            return Ok(false); // 已经在进行紧急ping
        }
        
        self.emergency_ping_active.store(true, Ordering::SeqCst);
        info!("触发紧急ping策略");
        
        let start_time = Instant::now();
        
        // 发送紧急ping
        let sink_result = {
            let mut sink = ws_sink.lock().await;
            sink.send(Message::Ping(vec![])).await
        };
        
        match sink_result {
            Ok(_) => {
                let mut last_ping = self.last_ping_time.write().await;
                *last_ping = start_time;
                info!("紧急ping发送成功");
                Ok(true)
            },
            Err(e) => {
                error!("紧急ping发送失败: {}", e);
                self.emergency_ping_active.store(false, Ordering::SeqCst);
                Err(ConnectorError::NetworkError(format!("紧急ping失败: {}", e)))
            }
        }
    }
    
    /// 处理pong响应
    pub async fn handle_pong(&self) -> Duration {
        let last_ping = self.last_ping_time.read().await;
        let latency = last_ping.elapsed();
        
        // 重置状态
        self.consecutive_timeouts.store(0, Ordering::SeqCst);
        self.emergency_ping_active.store(false, Ordering::SeqCst);
        
        debug!("收到pong响应，延迟: {:?}", latency);
        latency
    }
    
    /// 重置超时计数
    pub fn reset_timeout_count(&self) {
        self.consecutive_timeouts.store(0, Ordering::SeqCst);
        self.emergency_ping_active.store(false, Ordering::SeqCst);
    }
    
    /// 获取当前超时次数
    pub fn get_timeout_count(&self) -> usize {
        self.consecutive_timeouts.load(Ordering::SeqCst)
    }
    
    /// 检查是否正在进行紧急ping
    pub fn is_emergency_ping_active(&self) -> bool {
        self.emergency_ping_active.load(Ordering::SeqCst)
    }
}

/// 连接质量监控器
/// 实现连接质量评估和监控功能
#[derive(Debug)]
pub struct ConnectionQualityMonitor {
    latency_history: Arc<RwLock<Vec<Duration>>>,
    packet_loss_count: Arc<AtomicUsize>,
    total_packets: Arc<AtomicUsize>,
    last_update: Arc<RwLock<Instant>>,
    max_history_size: usize,
}

impl ConnectionQualityMonitor {
    pub fn new(max_history_size: usize) -> Self {
        Self {
            latency_history: Arc::new(RwLock::new(Vec::new())),
            packet_loss_count: Arc::new(AtomicUsize::new(0)),
            total_packets: Arc::new(AtomicUsize::new(0)),
            last_update: Arc::new(RwLock::new(Instant::now())),
            max_history_size,
        }
    }
    
    /// 记录延迟数据
    pub async fn record_latency(&self, latency: Duration) {
        let mut history = self.latency_history.write().await;
        history.push(latency);
        
        // 保持历史记录大小限制
        if history.len() > self.max_history_size {
            history.remove(0);
        }
        
        let mut last_update = self.last_update.write().await;
        *last_update = Instant::now();
    }
    
    /// 记录数据包丢失
    pub fn record_packet_loss(&self) {
        self.packet_loss_count.fetch_add(1, Ordering::SeqCst);
        self.total_packets.fetch_add(1, Ordering::SeqCst);
    }
    
    /// 记录成功数据包
    pub fn record_packet_success(&self) {
        self.total_packets.fetch_add(1, Ordering::SeqCst);
    }
    
    /// 获取连接质量报告
    pub async fn get_quality_report(&self) -> Result<ConnectionQuality, ConnectorError> {
        let history = self.latency_history.read().await;
        let packet_loss = self.packet_loss_count.load(Ordering::SeqCst);
        let total = self.total_packets.load(Ordering::SeqCst);
        
        // 计算平均延迟
        let avg_latency = if history.is_empty() {
            0.0
        } else {
            let total_ms: u64 = history.iter().map(|d| d.as_millis() as u64).sum();
            total_ms as f64 / history.len() as f64
        };
        
        // 计算丢包率
        let packet_loss_rate = if total == 0 {
            0.0
        } else {
            packet_loss as f64 / total as f64
        };
        
        // 计算稳定性评分 (基于延迟变化和丢包率)
        let stability_score = self.calculate_stability_score(&history, packet_loss_rate).await;
        
        Ok(ConnectionQuality {
            latency_ms: avg_latency,
            packet_loss_rate,
            stability_score,
            last_updated: chrono::Utc::now(),
        })
    }
    
    /// 计算稳定性评分
    async fn calculate_stability_score(&self, history: &[Duration], packet_loss_rate: f64) -> f64 {
        if history.len() < 2 {
            return 0.5; // 数据不足，返回中等评分
        }
        
        // 计算延迟变异系数
        let latencies: Vec<f64> = history.iter().map(|d| d.as_millis() as f64).collect();
        let mean = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let variance = latencies.iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>() / latencies.len() as f64;
        let std_dev = variance.sqrt();
        let cv = if mean > 0.0 { std_dev / mean } else { 0.0 };
        
        // 基于变异系数和丢包率计算稳定性
        let latency_stability = (1.0 - cv.min(1.0)).max(0.0);
        let packet_stability = (1.0 - packet_loss_rate * 10.0).max(0.0);
        
        // 综合评分
        (latency_stability * 0.7 + packet_stability * 0.3).min(1.0).max(0.0)
    }
    
    /// 重置统计数据
    pub async fn reset_stats(&self) {
        let mut history = self.latency_history.write().await;
        history.clear();
        self.packet_loss_count.store(0, Ordering::SeqCst);
        self.total_packets.store(0, Ordering::SeqCst);
        
        let mut last_update = self.last_update.write().await;
        *last_update = Instant::now();
    }
}

/// 自适应超时管理器
/// 根据网络状况动态调整超时时间
#[derive(Debug)]
pub struct AdaptiveTimeoutManager {
    base_timeout: Duration,
    current_timeout: Arc<RwLock<Duration>>,
    min_timeout: Duration,
    max_timeout: Duration,
    adjustment_factor: f64,
}

impl AdaptiveTimeoutManager {
    pub fn new(
        base_timeout: Duration,
        min_timeout: Duration,
        max_timeout: Duration,
        adjustment_factor: f64,
    ) -> Self {
        Self {
            base_timeout,
            current_timeout: Arc::new(RwLock::new(base_timeout)),
            min_timeout,
            max_timeout,
            adjustment_factor,
        }
    }
    
    /// 根据连接质量调整超时时间
    pub async fn adjust_timeout(&self, quality: &ConnectionQuality) {
        let mut current = self.current_timeout.write().await;
        
        // 基于延迟和稳定性调整超时
        let latency_factor = (quality.latency_ms / 100.0).max(0.5).min(3.0);
        let stability_factor = (2.0 - quality.stability_score).max(1.0).min(2.0);
        
        let new_timeout_ms = (self.base_timeout.as_millis() as f64 
            * latency_factor 
            * stability_factor 
            * self.adjustment_factor) as u64;
        
        let new_timeout = Duration::from_millis(new_timeout_ms)
            .max(self.min_timeout)
            .min(self.max_timeout);
        
        let diff = if *current > new_timeout {
            *current - new_timeout
        } else {
            new_timeout - *current
        };
        
        if diff > Duration::from_millis(100) {
            debug!("调整超时时间: {:?} -> {:?}", *current, new_timeout);
            *current = new_timeout;
        }
    }
    
    /// 获取当前超时时间
    pub async fn get_current_timeout(&self) -> Duration {
        *self.current_timeout.read().await
    }
    
    /// 重置为基础超时时间
    pub async fn reset_timeout(&self) {
        let mut current = self.current_timeout.write().await;
        *current = self.base_timeout;
    }
    
    /// 处理超时事件
    pub async fn handle_timeout(&self) {
        let mut current = self.current_timeout.write().await;
        let new_timeout = (current.as_millis() as f64 * 1.2) as u64;
        *current = Duration::from_millis(new_timeout).min(self.max_timeout);
    }
    
    /// 重置管理器状态
    pub async fn reset(&self) {
        self.reset_timeout().await;
    }
}