//! 连接器配置管理器
//! 
//! 提供统一的配置加载、验证和管理功能

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    fs,
};
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use log::{info, warn, error, debug};

/// 配置管理器错误类型
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("配置文件不存在: {path}")]
    FileNotFound { path: String },
    
    #[error("配置文件读取失败: {source}")]
    FileReadError {
        #[from]
        source: std::io::Error,
    },
    
    #[error("配置解析失败: {source}")]
    ParseError {
        #[from]
        source: toml::de::Error,
    },
    
    #[error("配置验证失败: {message}")]
    ValidationError { message: String },
    
    #[error("配置不存在: {key}")]
    ConfigNotFound { key: String },
    
    #[error("配置类型错误: {message}")]
    TypeError { message: String },
}

/// 配置源类型
#[derive(Debug, Clone)]
pub enum ConfigSource {
    /// 文件配置
    File(PathBuf),
    /// 环境变量配置
    Environment(String),
    /// 内存配置
    Memory(String),
}

/// 配置元数据
#[derive(Debug, Clone)]
pub struct ConfigMetadata {
    /// 配置源
    pub source: ConfigSource,
    /// 加载时间
    pub loaded_at: chrono::DateTime<chrono::Utc>,
    /// 配置版本
    pub version: Option<String>,
    /// 配置标签
    pub tags: HashMap<String, String>,
}

/// 配置条目
#[derive(Debug, Clone)]
struct ConfigEntry {
    /// 配置数据
    data: toml::Value,
    /// 配置元数据
    metadata: ConfigMetadata,
}

/// 配置管理器
#[derive(Debug)]
pub struct ConfigManager {
    /// 配置存储
    configs: HashMap<String, ConfigEntry>,
    /// 默认配置目录
    config_dir: PathBuf,
    /// 是否启用环境变量覆盖
    enable_env_override: bool,
}

impl ConfigManager {
    /// 创建新的配置管理器
    pub fn new<P: AsRef<Path>>(config_dir: P) -> Self {
        Self {
            configs: HashMap::new(),
            config_dir: config_dir.as_ref().to_path_buf(),
            enable_env_override: true,
        }
    }
    
    /// 设置是否启用环境变量覆盖
    pub fn with_env_override(mut self, enable: bool) -> Self {
        self.enable_env_override = enable;
        self
    }
    
    /// 从文件加载配置
    pub fn load_from_file<P: AsRef<Path>>(
        &mut self,
        key: String,
        file_path: P,
    ) -> Result<(), ConfigError> {
        let path = file_path.as_ref();
        
        if !path.exists() {
            return Err(ConfigError::FileNotFound {
                path: path.display().to_string(),
            });
        }
        
        let content = fs::read_to_string(path)?;
        let data: toml::Value = toml::from_str(&content)?;
        
        let metadata = ConfigMetadata {
            source: ConfigSource::File(path.to_path_buf()),
            loaded_at: chrono::Utc::now(),
            version: None,
            tags: HashMap::new(),
        };
        
        let entry = ConfigEntry { data, metadata };
        self.configs.insert(key.clone(), entry);
        
        info!("配置已加载: {} <- {}", key, path.display());
        Ok(())
    }
    
    /// 从TOML字符串加载配置
    pub fn load_from_string(
        &mut self,
        key: String,
        content: &str,
        source_name: String,
    ) -> Result<(), ConfigError> {
        let data: toml::Value = toml::from_str(content)?;
        
        let metadata = ConfigMetadata {
            source: ConfigSource::Memory(source_name),
            loaded_at: chrono::Utc::now(),
            version: None,
            tags: HashMap::new(),
        };
        
        let entry = ConfigEntry { data, metadata };
        self.configs.insert(key.clone(), entry);
        
        info!("配置已加载: {key} <- 内存");
        Ok(())
    }
    
    /// 从环境变量加载配置
    pub fn load_from_env(
        &mut self,
        key: String,
        env_prefix: &str,
    ) -> Result<(), ConfigError> {
        let mut config_map = HashMap::new();
        
        for (env_key, env_value) in std::env::vars() {
            if env_key.starts_with(env_prefix) {
                let config_key = env_key
                    .strip_prefix(env_prefix)
                    .unwrap()
                    .trim_start_matches('_')
                    .to_lowercase();
                
                config_map.insert(config_key, toml::Value::String(env_value));
            }
        }
        
        let data = toml::Value::Table(config_map.into_iter().collect());
        
        let metadata = ConfigMetadata {
            source: ConfigSource::Environment(env_prefix.to_string()),
            loaded_at: chrono::Utc::now(),
            version: None,
            tags: HashMap::new(),
        };
        
        let entry = ConfigEntry { data, metadata };
        self.configs.insert(key.clone(), entry);
        
        info!("配置已从环境变量加载: {key} <- {env_prefix}");
        Ok(())
    }
    
    /// 获取配置
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T, ConfigError> {
        let entry = self.configs.get(key)
            .ok_or_else(|| ConfigError::ConfigNotFound { key: key.to_string() })?;
        
        let config: T = entry.data.clone().try_into()
            .map_err(|e| ConfigError::TypeError { 
                message: format!("配置类型转换失败: {e}") 
            })?;
        
        Ok(config)
    }
    
    /// 获取配置的原始值
    pub fn get_raw(&self, key: &str) -> Result<&toml::Value, ConfigError> {
        let entry = self.configs.get(key)
            .ok_or_else(|| ConfigError::ConfigNotFound { key: key.to_string() })?;
        
        Ok(&entry.data)
    }
    
    /// 获取配置元数据
    pub fn get_metadata(&self, key: &str) -> Result<&ConfigMetadata, ConfigError> {
        let entry = self.configs.get(key)
            .ok_or_else(|| ConfigError::ConfigNotFound { key: key.to_string() })?;
        
        Ok(&entry.metadata)
    }
    
    /// 检查配置是否存在
    pub fn contains(&self, key: &str) -> bool {
        self.configs.contains_key(key)
    }
    
    /// 列出所有配置键
    pub fn list_keys(&self) -> Vec<String> {
        self.configs.keys().cloned().collect()
    }
    
    /// 移除配置
    pub fn remove(&mut self, key: &str) -> Result<(), ConfigError> {
        match self.configs.remove(key) {
            Some(_) => {
                info!("配置已移除: {key}");
                Ok(())
            }
            None => Err(ConfigError::ConfigNotFound { key: key.to_string() }),
        }
    }
    
    /// 验证配置
    pub fn validate<T, F>(&self, key: &str, validator: F) -> Result<(), ConfigError>
    where
        T: DeserializeOwned,
        F: FnOnce(&T) -> Result<(), String>,
    {
        let config: T = self.get(key)?;
        
        validator(&config).map_err(|message| ConfigError::ValidationError { message })
    }
    
    /// 合并配置
    pub fn merge(&mut self, other_key: &str, target_key: &str) -> Result<(), ConfigError> {
        let other_entry = self.configs.get(other_key)
            .ok_or_else(|| ConfigError::ConfigNotFound { key: other_key.to_string() })?
            .clone();
        
        let target_entry = self.configs.get_mut(target_key)
            .ok_or_else(|| ConfigError::ConfigNotFound { key: target_key.to_string() })?;
        
        // 简单的合并逻辑：如果都是表，则合并键值对
        if let (toml::Value::Table(ref mut target_table), toml::Value::Table(other_table)) = 
            (&mut target_entry.data, &other_entry.data) {
            for (key, value) in other_table {
                target_table.insert(key.clone(), value.clone());
            }
            
            info!("配置已合并: {other_key} -> {target_key}");
        } else {
            return Err(ConfigError::TypeError {
                message: "只能合并表类型的配置".to_string(),
            });
        }
        
        Ok(())
    }
    
    /// 自动加载目录中的所有配置文件
    pub fn auto_load_directory(&mut self) -> Result<Vec<String>, ConfigError> {
        let mut loaded_keys = Vec::new();
        
        if !self.config_dir.exists() {
            warn!("配置目录不存在: {}", self.config_dir.display());
            return Ok(loaded_keys);
        }
        
        let entries = fs::read_dir(&self.config_dir)?;
        
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.extension().is_some_and(|ext| ext == "toml") {
                if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let key = file_stem.to_string();
                    
                    match self.load_from_file(key.clone(), &path) {
                        Ok(_) => {
                            loaded_keys.push(key);
                        }
                        Err(e) => {
                            error!("加载配置文件失败 {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }
        
        info!("自动加载了 {} 个配置文件", loaded_keys.len());
        Ok(loaded_keys)
    }
    
    /// 重新加载配置
    pub fn reload(&mut self, key: &str) -> Result<(), ConfigError> {
        // 先获取配置源信息，避免借用冲突
        let source = {
            let entry = self.configs.get(key)
                .ok_or_else(|| ConfigError::ConfigNotFound { key: key.to_string() })?;
            entry.metadata.source.clone()
        };
        
        match source {
            ConfigSource::File(path) => {
                self.load_from_file(key.to_string(), &path)?;
                info!("配置已重新加载: {key}");
            }
            ConfigSource::Environment(prefix) => {
                self.load_from_env(key.to_string(), &prefix)?;
                info!("配置已从环境变量重新加载: {key}");
            }
            ConfigSource::Memory(_) => {
                warn!("内存配置无法重新加载: {key}");
            }
        }
        
        Ok(())
    }
}

/// 默认配置管理器实例
static DEFAULT_CONFIG_MANAGER: std::sync::Mutex<Option<ConfigManager>> = std::sync::Mutex::new(None);
static INIT: std::sync::Once = std::sync::Once::new();

/// 获取默认配置管理器
pub fn default_config_manager() -> std::sync::MutexGuard<'static, Option<ConfigManager>> {
    INIT.call_once(|| {
        let config_dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("config");
        
        let mut manager = DEFAULT_CONFIG_MANAGER.lock().unwrap();
        *manager = Some(ConfigManager::new(config_dir));
    });
    
    DEFAULT_CONFIG_MANAGER.lock().unwrap()
}

/// 配置宏，用于简化配置获取
#[macro_export]
macro_rules! get_config {
    ($key:expr, $type:ty) => {
        $crate::connectors::managers::config_manager::default_config_manager()
            .as_ref().unwrap().get::<$type>($key)
    };
}

/// 配置验证宏
#[macro_export]
macro_rules! validate_config {
    ($key:expr, $type:ty, $validator:expr) => {
        $crate::connectors::managers::config_manager::default_config_manager()
            .as_mut().unwrap().validate::<$type, _>($key, $validator)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestConfig {
        name: String,
        port: u16,
        enabled: bool,
    }
    
    #[test]
    fn test_config_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ConfigManager::new(temp_dir.path());
        
        assert_eq!(manager.list_keys().len(), 0);
    }
    
    #[test]
    fn test_load_from_string() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = ConfigManager::new(temp_dir.path());
        
        let config_content = r#"
            name = "test"
            port = 8080
            enabled = true
        "#;
        
        let result = manager.load_from_string(
            "test".to_string(),
            config_content,
            "test_source".to_string(),
        );
        
        assert!(result.is_ok());
        assert!(manager.contains("test"));
        
        let config: TestConfig = manager.get("test").unwrap();
        assert_eq!(config.name, "test");
        assert_eq!(config.port, 8080);
        assert!(config.enabled);
    }
    
    #[test]
    fn test_config_validation() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = ConfigManager::new(temp_dir.path());
        
        let config_content = r#"
            name = "test"
            port = 8080
            enabled = true
        "#;
        
        manager.load_from_string(
            "test".to_string(),
            config_content,
            "test_source".to_string(),
        ).unwrap();
        
        // 验证成功的情况
        let result = manager.validate::<TestConfig, _>("test", |config| {
            if config.port > 1024 {
                Ok(())
            } else {
                Err("端口必须大于1024".to_string())
            }
        });
        
        assert!(result.is_ok());
        
        // 验证失败的情况
        let result = manager.validate::<TestConfig, _>("test", |config| {
            if config.port < 1024 {
                Ok(())
            } else {
                Err("端口必须小于1024".to_string())
            }
        });
        
        assert!(result.is_err());
    }
    
    #[test]
    fn test_config_removal() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = ConfigManager::new(temp_dir.path());
        
        let config_content = r#"
            name = "test"
            port = 8080
            enabled = true
        "#;
        
        manager.load_from_string(
            "test".to_string(),
            config_content,
            "test_source".to_string(),
        ).unwrap();
        
        assert!(manager.contains("test"));
        
        let result = manager.remove("test");
        assert!(result.is_ok());
        assert!(!manager.contains("test"));
    }
}