// src/sinks/file_sink.rs - 文件存储Sink实现

use super::{DataSink, SinkHealth, SinkMetrics};
use crate::types::errors::AppError;
use async_trait::async_trait;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Instant;
use tokio::sync::Mutex;

// 重新导出config.rs中的FileSinkConfig
pub use crate::config::FileSinkConfig;

// 为了向后兼容，提供默认实现
impl Default for FileSinkConfig {
    fn default() -> Self {
        Self {
            data_dir: "./data".to_string(),
            batch_size: 128,
            flush_interval_ms: 1000,
            rotation_size_bytes: Some(100 * 1024 * 1024), // 100MB
            enable_compression: false,
        }
    }
}

/// 文件存储Sink实现
pub struct FileSink {
    config: FileSinkConfig,
    writer: Mutex<BufWriter<std::fs::File>>,
    buffer: Mutex<Vec<String>>,
    metrics: Mutex<SinkMetrics>,
    health: Mutex<SinkHealth>,
    last_flush: Mutex<Instant>,
}

impl FileSink {
    pub fn new(config: FileSinkConfig) -> Result<Self, AppError> {
        // 确保数据目录存在
        std::fs::create_dir_all(&config.data_dir)
            .map_err(|e| AppError::IoError(e.to_string()))?;
        
        // 生成文件路径
        let file_path = format!("{}/market_data.jsonl", config.data_dir);
        
        // 打开文件
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .map_err(|e| AppError::IoError(e.to_string()))?;
        
        let writer = BufWriter::new(file);
        
        Ok(Self {
            config,
            writer: Mutex::new(writer),
            buffer: Mutex::new(Vec::with_capacity(1000)),
            metrics: Mutex::new(SinkMetrics::new()),
            health: Mutex::new(SinkHealth::new()),
            last_flush: Mutex::new(Instant::now()),
        })
    }
    
    /// 检查是否需要刷写
    async fn should_flush(&self) -> bool {
        let buffer = self.buffer.lock().await;
        let last_flush = self.last_flush.lock().await;
        
        buffer.len() >= self.config.batch_size ||
        last_flush.elapsed().as_millis() >= self.config.flush_interval_ms as u128
    }
    
    /// 内部刷写实现
    async fn internal_flush(&self) -> Result<usize, AppError> {
        let start_time = Instant::now();
        let mut buffer = self.buffer.lock().await;
        let mut writer = self.writer.lock().await;
        let mut metrics = self.metrics.lock().await;
        let mut health = self.health.lock().await;
        let mut last_flush = self.last_flush.lock().await;
        
        if buffer.is_empty() {
            return Ok(0);
        }
        
        let record_count = buffer.len();
        let mut total_bytes = 0;
        
        // 写入所有缓冲的记录
        for record in buffer.iter() {
            let bytes = record.as_bytes();
            writer.write_all(bytes)
                .map_err(|e| AppError::IoError(e.to_string()))?;
            writer.write_all(b"\n")
                .map_err(|e| AppError::IoError(e.to_string()))?;
            total_bytes += bytes.len() + 1; // +1 for newline
        }
        
        // 刷写到磁盘
        writer.flush()
            .map_err(|e| AppError::IoError(e.to_string()))?;
        
        // 清空缓冲区
        buffer.clear();
        
        // 更新指标
        let latency_ms = start_time.elapsed().as_millis() as f64;
        metrics.record_write(record_count, total_bytes, latency_ms);
        metrics.record_flush();
        
        // 更新健康状态
        health.mark_write_success();
        
        // 更新最后刷写时间
        *last_flush = Instant::now();
        
        Ok(record_count)
    }
}

#[async_trait]
impl DataSink for FileSink {
    async fn initialize(&mut self) -> Result<(), AppError> {
        // 文件Sink在创建时已经初始化
        Ok(())
    }
    
    async fn write_record_json(
        &mut self,
        json_str: &str,
    ) -> Result<(), AppError> {
        {
            let mut buffer = self.buffer.lock().await;
            buffer.push(json_str.to_string());
        }
        
        // 检查是否需要自动刷写
        if self.should_flush().await {
            self.internal_flush().await?;
        }
        
        Ok(())
    }
    
    async fn write_batch_json(
        &mut self,
        json_strings: &[String],
    ) -> Result<usize, AppError> {
        if json_strings.is_empty() {
            return Ok(0);
        }
        
        let start_time = Instant::now();
        
        // 添加到缓冲区
        {
            let mut buffer = self.buffer.lock().await;
            buffer.extend_from_slice(json_strings);
        }
        
        // 检查是否需要刷写
        if self.should_flush().await {
            self.internal_flush().await?;
        }
        
        // 记录批量写入指标
        let latency_ms = start_time.elapsed().as_millis() as f64;
        let mut metrics = self.metrics.lock().await;
        metrics.record_write(json_strings.len(), 0, latency_ms); // bytes will be counted in flush
        
        Ok(json_strings.len())
    }
    
    async fn flush(&mut self) -> Result<(), AppError> {
        self.internal_flush().await?;
        Ok(())
    }
    
    async fn health_check(&self) -> Result<SinkHealth, AppError> {
        let health = self.health.lock().await;
        
        // 检查文件是否可写
        let mut health_check = health.clone();
        
        // 检查数据目录是否存在且可写
        if !Path::new(&self.config.data_dir).exists() {
            health_check = health_check.with_error("数据目录不存在");
        }
        
        // 检查最后写入时间
        if let Some(last_write) = health_check.last_write_time {
            if last_write.elapsed().as_secs() > 300 { // 5分钟没有写入
                health_check.details.insert(
                    "warning".to_string(),
                    "超过5分钟没有写入数据".to_string()
                );
            }
        }
        
        Ok(health_check)
    }
    
    fn get_metrics(&self) -> SinkMetrics {
        // 由于这是同步方法，我们需要使用try_lock
        if let Ok(metrics) = self.metrics.try_lock() {
            let mut result = metrics.clone();
            
            // 添加当前缓冲区大小
            if let Ok(buffer) = self.buffer.try_lock() {
                result.buffer_size = buffer.len();
            }
            
            result
        } else {
            SinkMetrics::new()
        }
    }
    
    async fn close(&mut self) -> Result<(), AppError> {
        // 刷写剩余数据
        self.internal_flush().await?;
        
        // 关闭文件
        let mut writer = self.writer.lock().await;
        writer.flush()
            .map_err(|e| AppError::IoError(e.to_string()))?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::NamedTempFile;
    
    #[tokio::test]
    async fn test_file_sink_basic_operations() {
        let temp_file = NamedTempFile::new().unwrap();
        let config = FileSinkConfig {
            data_dir: temp_file.path().parent().unwrap().to_string_lossy().to_string(),
            batch_size: 2,
            flush_interval_ms: 1000,
            rotation_size_bytes: None,
            enable_compression: false,
        };
        
        let mut sink = FileSink::new(config).unwrap();
        
        // 测试初始化
        sink.initialize().await.unwrap();
        
        // 测试写入单条记录（使用便利函数）
        let record = json!({"test": "data", "timestamp": 1234567890});
        crate::sinks::write_record_serialized(&mut sink, &record).await.unwrap();
        
        // 测试批量写入（使用便利函数）
        let records = vec![
            json!({"test": "batch1", "timestamp": 1234567891}),
            json!({"test": "batch2", "timestamp": 1234567892}),
        ];
        crate::sinks::write_batch_serialized(&mut sink, &records).await.unwrap();
        
        // 测试直接JSON写入
        sink.write_record_json("{\"test\": \"direct_json\", \"timestamp\": 1234567893}").await.unwrap();
        
        // 测试批量JSON写入
        let json_strings = vec![
            "{\"test\": \"json_batch1\", \"timestamp\": 1234567894}".to_string(),
            "{\"test\": \"json_batch2\", \"timestamp\": 1234567895}".to_string(),
        ];
        sink.write_batch_json(&json_strings).await.unwrap();
        
        // 测试刷写
        sink.flush().await.unwrap();
        
        // 测试健康检查
        let health = sink.health_check().await.unwrap();
        assert!(health.is_healthy);
        
        // 测试指标
        let metrics = sink.get_metrics();
        assert!(metrics.records_written > 0);
        
        // 测试关闭
        sink.close().await.unwrap();
    }
}