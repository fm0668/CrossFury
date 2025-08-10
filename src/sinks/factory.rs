// src/sinks/factory.rs - Sink工厂实现

use super::{DataSink, SinkType};
use super::file_sink::{FileSink, FileSinkConfig};
use crate::types::errors::AppError;
use crate::config::SinkConfiguration;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Sink配置枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SinkConfig {
    File {
        data_dir: String,
        batch_size: Option<usize>,
        flush_interval_ms: Option<u64>,
        rotation_size_bytes: Option<u64>,
        enable_compression: Option<bool>,
    },
    Kafka {
        brokers: String,
        topic: String,
        batch_size: Option<usize>,
        compression: Option<String>,
        security_protocol: Option<String>,
    },
    ClickHouse {
        url: String,
        database: String,
        table: String,
        batch_size: Option<usize>,
        username: Option<String>,
        password: Option<String>,
    },
    S3 {
        bucket: String,
        prefix: String,
        region: String,
        access_key: Option<String>,
        secret_key: Option<String>,
        batch_size: Option<usize>,
    },
}

impl SinkConfig {
    /// 获取Sink类型
    pub fn get_type(&self) -> SinkType {
        match self {
            SinkConfig::File { .. } => SinkType::File,
            SinkConfig::Kafka { .. } => SinkType::Kafka,
            SinkConfig::ClickHouse { .. } => SinkType::ClickHouse,
            SinkConfig::S3 { .. } => SinkType::S3,
        }
    }
    
    /// 验证配置
    pub fn validate(&self) -> Result<(), AppError> {
        match self {
            SinkConfig::File { data_dir, .. } => {
                if data_dir.is_empty() {
                    return Err(AppError::ConfigError("数据目录不能为空".to_string()));
                }
            },
            SinkConfig::Kafka { brokers, topic, .. } => {
                if brokers.is_empty() {
                    return Err(AppError::ConfigError("Kafka brokers不能为空".to_string()));
                }
                if topic.is_empty() {
                    return Err(AppError::ConfigError("Kafka topic不能为空".to_string()));
                }
            },
            SinkConfig::ClickHouse { url, database, table, .. } => {
                if url.is_empty() {
                    return Err(AppError::ConfigError("ClickHouse URL不能为空".to_string()));
                }
                if database.is_empty() {
                    return Err(AppError::ConfigError("ClickHouse数据库名不能为空".to_string()));
                }
                if table.is_empty() {
                    return Err(AppError::ConfigError("ClickHouse表名不能为空".to_string()));
                }
            },
            SinkConfig::S3 { bucket, region, .. } => {
                if bucket.is_empty() {
                    return Err(AppError::ConfigError("S3 bucket不能为空".to_string()));
                }
                if region.is_empty() {
                    return Err(AppError::ConfigError("S3 region不能为空".to_string()));
                }
            },
        }
        Ok(())
    }
}

/// Sink工厂
pub struct SinkFactory;

impl SinkFactory {
    /// 从主配置文件的SinkConfiguration创建Sink实例
    pub fn create_sink_from_config(config: &SinkConfiguration) -> Result<Box<dyn DataSink>, AppError> {
        match config.sink_type.as_str() {
            "file" => {
                if let Some(file_config) = &config.file {
                    let sink = FileSink::new(file_config.clone())?;
                    Ok(Box::new(sink))
                } else {
                    Err(AppError::ConfigError("文件Sink配置缺失".to_string()))
                }
            },
            "kafka" => {
                // TODO: 实现Kafka Sink
                Err(AppError::ConfigError("Kafka Sink尚未实现".to_string()))
            },
            "clickhouse" => {
                // TODO: 实现ClickHouse Sink
                Err(AppError::ConfigError("ClickHouse Sink尚未实现".to_string()))
            },
            "s3" => {
                // TODO: 实现S3 Sink
                Err(AppError::ConfigError("S3 Sink尚未实现".to_string()))
            },
            _ => Err(AppError::ConfigError(format!("不支持的Sink类型: {}", config.sink_type)))
        }
    }
    
    /// 创建Sink实例（使用工厂内部配置格式）
    pub fn create_sink(config: SinkConfig) -> Result<Box<dyn DataSink>, AppError> {
        // 验证配置
        config.validate()?;
        
        match config {
            SinkConfig::File {
                data_dir,
                batch_size,
                flush_interval_ms,
                rotation_size_bytes,
                enable_compression,
            } => {
                let file_config = FileSinkConfig {
                    data_dir,
                    batch_size: batch_size.unwrap_or(128),
                    flush_interval_ms: flush_interval_ms.unwrap_or(1000),
                    rotation_size_bytes,
                    enable_compression: enable_compression.unwrap_or(false),
                };
                
                let sink = FileSink::new(file_config)?;
                Ok(Box::new(sink))
            },
            SinkConfig::Kafka { .. } => {
                // TODO: 实现Kafka Sink
                Err(AppError::ConfigError("Kafka Sink尚未实现".to_string()))
            },
            SinkConfig::ClickHouse { .. } => {
                // TODO: 实现ClickHouse Sink
                Err(AppError::ConfigError("ClickHouse Sink尚未实现".to_string()))
            },
            SinkConfig::S3 { .. } => {
                // TODO: 实现S3 Sink
                Err(AppError::ConfigError("S3 Sink尚未实现".to_string()))
            },
        }
    }
    
    /// 从配置文件创建多个Sink
    pub fn create_sinks_from_config(
        configs: HashMap<String, SinkConfig>
    ) -> Result<HashMap<String, Box<dyn DataSink>>, AppError> {
        let mut sinks = HashMap::new();
        
        for (name, config) in configs {
            let sink = Self::create_sink(config)?;
            sinks.insert(name, sink);
        }
        
        Ok(sinks)
    }
    
    /// 获取支持的Sink类型列表
    pub fn supported_types() -> Vec<SinkType> {
        vec![
            SinkType::File,
            // SinkType::Kafka,     // TODO: 待实现
            // SinkType::ClickHouse, // TODO: 待实现
            // SinkType::S3,         // TODO: 待实现
        ]
    }
    
    /// 检查Sink类型是否支持
    pub fn is_supported(sink_type: &SinkType) -> bool {
        Self::supported_types().contains(sink_type)
    }
}

/// 默认Sink配置
impl Default for SinkConfig {
    fn default() -> Self {
        SinkConfig::File {
            data_dir: "./data".to_string(),
            batch_size: Some(128),
            flush_interval_ms: Some(1000),
            rotation_size_bytes: Some(100 * 1024 * 1024), // 100MB
            enable_compression: Some(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_sink_config_validation() {
        // 测试有效的文件配置
        let valid_config = SinkConfig::File {
            data_dir: "/tmp".to_string(),
            batch_size: Some(1000),
            flush_interval_ms: Some(5000),
            rotation_size_bytes: None,
            enable_compression: Some(false),
        };
        assert!(valid_config.validate().is_ok());
        
        // 测试无效的文件配置
        let invalid_config = SinkConfig::File {
            data_dir: "".to_string(),
            batch_size: Some(1000),
            flush_interval_ms: Some(5000),
            rotation_size_bytes: None,
            enable_compression: Some(false),
        };
        assert!(invalid_config.validate().is_err());
    }
    
    #[test]
    fn test_sink_factory_create_file_sink() {
        let temp_file = NamedTempFile::new().unwrap();
        let config = SinkConfig::File {
            data_dir: temp_file.path().parent().unwrap().to_string_lossy().to_string(),
            batch_size: Some(500),
            flush_interval_ms: Some(3000),
            rotation_size_bytes: None,
            enable_compression: Some(false),
        };
        
        let sink = SinkFactory::create_sink(config);
        assert!(sink.is_ok());
    }
    
    #[test]
    fn test_supported_types() {
        let types = SinkFactory::supported_types();
        assert!(types.contains(&SinkType::File));
        assert!(SinkFactory::is_supported(&SinkType::File));
    }
    
    #[test]
    fn test_sink_config_serialization() {
        let config = SinkConfig::File {
            data_dir: "/tmp".to_string(),
            batch_size: Some(1000),
            flush_interval_ms: Some(5000),
            rotation_size_bytes: Some(100 * 1024 * 1024),
            enable_compression: Some(false),
        };
        
        // 测试序列化
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("file"));
        assert!(json.contains("/tmp"));
        
        // 测试反序列化
        let deserialized: SinkConfig = serde_json::from_str(&json).unwrap();
        match deserialized {
            SinkConfig::File { data_dir, .. } => {
                assert_eq!(data_dir, "/tmp");
            },
            _ => panic!("反序列化类型错误"),
        }
    }
}