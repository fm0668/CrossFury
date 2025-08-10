// src/sinks/mod.rs - 可插拔数据落地抽象

use async_trait::async_trait;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Instant;
use crate::core::AppError;

pub mod file_sink;
pub mod factory;

/// 数据落地抽象接口
#[async_trait]
pub trait DataSink: Send + Sync {
    /// 初始化 Sink
    async fn initialize(&mut self) -> Result<(), AppError>;
    
    /// 写入单条记录（JSON字符串）
    async fn write_record_json(
        &mut self,
        json_str: &str,
    ) -> Result<(), AppError>;
    
    /// 批量写入记录（JSON字符串数组）
    async fn write_batch_json(
        &mut self,
        json_strings: &[String],
    ) -> Result<usize, AppError>;
    
    /// 强制刷写缓冲区
    async fn flush(&mut self) -> Result<(), AppError>;
    
    /// 健康检查
    async fn health_check(&self) -> Result<SinkHealth, AppError>;
    
    /// 获取指标
    fn get_metrics(&self) -> SinkMetrics;
    
    /// 关闭 Sink
    async fn close(&mut self) -> Result<(), AppError>;
}

/// 便利函数，用于序列化并写入单条记录
pub async fn write_record_serialized<T: Serialize + Send + Sync>(
    sink: &mut dyn DataSink,
    record: &T,
) -> Result<(), AppError> {
    let json_str = serde_json::to_string(record)
        .map_err(AppError::SerializationError)?;
    sink.write_record_json(&json_str).await
}

/// 便利函数，用于序列化并批量写入记录
pub async fn write_batch_serialized<T: Serialize + Send + Sync>(
    sink: &mut dyn DataSink,
    records: &[T],
) -> Result<usize, AppError> {
    let mut json_strings = Vec::with_capacity(records.len());
    for record in records {
        let json_str = serde_json::to_string(record)
            .map_err(AppError::SerializationError)?;
        json_strings.push(json_str);
    }
    sink.write_batch_json(&json_strings).await
}

/// Sink 健康状态
#[derive(Debug, Clone)]
pub struct SinkHealth {
    pub is_healthy: bool,
    pub last_write_time: Option<Instant>,
    pub error_count: u64,
    pub details: HashMap<String, String>,
}

impl SinkHealth {
    pub fn new() -> Self {
        Self {
            is_healthy: true,
            last_write_time: None,
            error_count: 0,
            details: HashMap::new(),
        }
    }
    
    pub fn with_error(mut self, error: &str) -> Self {
        self.is_healthy = false;
        self.error_count += 1;
        self.details.insert("last_error".to_string(), error.to_string());
        self
    }
    
    pub fn mark_write_success(&mut self) {
        self.last_write_time = Some(Instant::now());
        self.is_healthy = true;
    }
}

/// Sink 指标
#[derive(Debug, Clone, Default)]
pub struct SinkMetrics {
    pub records_written: u64,
    pub bytes_written: u64,
    pub write_errors: u64,
    pub flush_count: u64,
    pub avg_write_latency_ms: f64,
    pub buffer_size: usize,
}

impl SinkMetrics {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn record_write(&mut self, record_count: usize, bytes: usize, latency_ms: f64) {
        self.records_written += record_count as u64;
        self.bytes_written += bytes as u64;
        
        // 计算移动平均延迟
        if self.records_written == record_count as u64 {
            self.avg_write_latency_ms = latency_ms;
        } else {
            self.avg_write_latency_ms = 
                (self.avg_write_latency_ms * 0.9) + (latency_ms * 0.1);
        }
    }
    
    pub fn record_error(&mut self) {
        self.write_errors += 1;
    }
    
    pub fn record_flush(&mut self) {
        self.flush_count += 1;
    }
}

/// Sink 类型枚举
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SinkType {
    File,
    Kafka,
    ClickHouse,
    S3,
}

impl std::str::FromStr for SinkType {
    type Err = AppError;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "file" => Ok(SinkType::File),
            "kafka" => Ok(SinkType::Kafka),
            "clickhouse" => Ok(SinkType::ClickHouse),
            "s3" => Ok(SinkType::S3),
            _ => Err(AppError::ConfigError(format!("未知的Sink类型: {}", s))),
        }
    }
}

impl std::fmt::Display for SinkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SinkType::File => write!(f, "file"),
            SinkType::Kafka => write!(f, "kafka"),
            SinkType::ClickHouse => write!(f, "clickhouse"),
            SinkType::S3 => write!(f, "s3"),
        }
    }
}