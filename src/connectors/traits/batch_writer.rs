//! 批量数据写入器模块
//! 
//! 提供高性能的批量数据写入功能，支持缓冲和批量处理

use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc, RwLock, Mutex},
    time::interval,
};
use serde::Serialize;
use log::{debug, error};

/// 批量写入配置
#[derive(Debug, Clone)]
pub struct BatchWriterConfig {
    /// 批量大小
    pub batch_size: usize,
    /// 刷新间隔
    pub flush_interval: Duration,
    /// 最大缓冲区大小
    pub max_buffer_size: usize,
    /// 写入超时
    pub write_timeout: Duration,
}

impl Default for BatchWriterConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            flush_interval: Duration::from_millis(500),
            max_buffer_size: 10000,
            write_timeout: Duration::from_secs(5),
        }
    }
}

/// 批量写入项
#[derive(Debug, Clone)]
pub struct BatchItem<T> {
    /// 数据
    pub data: T,
    /// 时间戳
    pub timestamp: Instant,
    /// 重试次数
    pub retry_count: u32,
}

impl<T> BatchItem<T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            timestamp: Instant::now(),
            retry_count: 0,
        }
    }
}

/// 批量写入器trait
#[async_trait::async_trait]
pub trait BatchWriter<T>: Send + Sync 
where
    T: Send + Sync + 'static,
{
    type Error: std::error::Error + Send + Sync + 'static;
    
    /// 写入批量数据
    async fn write_batch(&self, items: Vec<BatchItem<T>>) -> Result<(), Self::Error>;
    
    /// 获取写入器名称
    fn name(&self) -> &str;
}

/// 批量数据处理器
pub struct BatchProcessor<T, W> 
where
    T: Send + Sync + 'static,
    W: BatchWriter<T> + 'static,
{
    /// 配置
    config: BatchWriterConfig,
    /// 写入器
    writer: Arc<W>,
    /// 数据缓冲区
    buffer: Arc<Mutex<VecDeque<BatchItem<T>>>>,
    /// 接收器
    receiver: Arc<Mutex<Option<mpsc::Receiver<BatchItem<T>>>>>,
    /// 发送器
    sender: mpsc::Sender<BatchItem<T>>,
    /// 运行状态
    running: Arc<RwLock<bool>>,
}

impl<T, W> BatchProcessor<T, W>
where
    T: Send + Sync + 'static,
    W: BatchWriter<T> + 'static,
{
    /// 创建新的批量处理器
    pub fn new(config: BatchWriterConfig, writer: W) -> Self {
        let (sender, receiver) = mpsc::channel(config.max_buffer_size);
        
        Self {
            config,
            writer: Arc::new(writer),
            buffer: Arc::new(Mutex::new(VecDeque::new())),
            receiver: Arc::new(Mutex::new(Some(receiver))),
            sender,
            running: Arc::new(RwLock::new(false)),
        }
    }
    
    /// 获取发送器
    pub fn sender(&self) -> mpsc::Sender<BatchItem<T>> {
        self.sender.clone()
    }
    
    /// 启动批量处理器
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut running = self.running.write().await;
        if *running {
            return Ok(());
        }
        *running = true;
        drop(running);
        
        // 取出接收器
        let receiver = {
            let mut recv_guard = self.receiver.lock().await;
            recv_guard.take().ok_or("BatchProcessor already started")?  
        };
        
        // 启动处理任务
        let buffer = self.buffer.clone();
        let writer = self.writer.clone();
        let config = self.config.clone();
        let running = self.running.clone();
        
        tokio::spawn(async move {
            Self::process_loop(receiver, buffer, writer, config, running).await;
        });
        
        Ok(())
    }
    
    /// 停止批量处理器
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
    }
    
    /// 处理循环
    async fn process_loop(
        mut receiver: mpsc::Receiver<BatchItem<T>>,
        buffer: Arc<Mutex<VecDeque<BatchItem<T>>>>,
        writer: Arc<W>,
        config: BatchWriterConfig,
        running: Arc<RwLock<bool>>,
    ) {
        let mut flush_timer = interval(config.flush_interval);
        
        loop {
            tokio::select! {
                // 接收新数据
                item = receiver.recv() => {
                    match item {
                        Some(item) => {
                            let mut buf = buffer.lock().await;
                            buf.push_back(item);
                            
                            // 检查是否需要立即刷新
                            if buf.len() >= config.batch_size {
                                let items: Vec<_> = buf.drain(..config.batch_size).collect();
                                drop(buf);
                                
                                if let Err(e) = writer.write_batch(items).await {
                                    error!("批量写入失败: {e}");
                                }
                            }
                        }
                        None => {
                            debug!("接收器关闭，退出处理循环");
                            break;
                        }
                    }
                }
                
                // 定时刷新
                _ = flush_timer.tick() => {
                    let mut buf = buffer.lock().await;
                    if !buf.is_empty() {
                        let items: Vec<_> = buf.drain(..).collect();
                        drop(buf);
                        
                        if let Err(e) = writer.write_batch(items).await {
                            error!("定时批量写入失败: {e}");
                        }
                    }
                }
            }
            
            // 检查运行状态
            if !*running.read().await {
                debug!("批量处理器停止");
                break;
            }
        }
        
        // 处理剩余数据
        let mut buf = buffer.lock().await;
        if !buf.is_empty() {
            let items: Vec<_> = buf.drain(..).collect();
            drop(buf);
            
            if let Err(e) = writer.write_batch(items).await {
                error!("最终批量写入失败: {e}");
            }
        }
    }
    
    /// 添加数据到批量处理器
    pub async fn add(&self, data: T) -> Result<(), mpsc::error::SendError<BatchItem<T>>> {
        let item = BatchItem::new(data);
        self.sender.send(item).await
    }
    
    /// 获取缓冲区大小
    pub async fn buffer_size(&self) -> usize {
        self.buffer.lock().await.len()
    }
    
    /// 强制刷新缓冲区
    pub async fn flush(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut buf = self.buffer.lock().await;
        if !buf.is_empty() {
            let items: Vec<_> = buf.drain(..).collect();
            drop(buf);
            
            self.writer.write_batch(items).await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        }
        Ok(())
    }
}

/// 文件批量写入器
pub struct FileBatchWriter {
    /// 文件路径
    file_path: String,
    /// 写入器名称
    name: String,
}

impl FileBatchWriter {
    pub fn new(file_path: String, name: String) -> Self {
        Self { file_path, name }
    }
}

#[async_trait::async_trait]
impl<T> BatchWriter<T> for FileBatchWriter
where
    T: Serialize + Send + Sync + 'static,
{
    type Error = std::io::Error;
    
    async fn write_batch(&self, items: Vec<BatchItem<T>>) -> Result<(), Self::Error> {
        use tokio::fs::OpenOptions;
        use tokio::io::AsyncWriteExt;
        
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .await?;
        
        for item in items {
            let json_line = serde_json::to_string(&item.data)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            
            file.write_all(json_line.as_bytes()).await?;
            file.write_all(b"\n").await?;
        }
        
        file.flush().await?;
        Ok(())
    }
    
    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;
    
    #[derive(Debug, Clone, Serialize)]
    struct TestData {
        id: u64,
        value: String,
    }
    
    #[tokio::test]
    async fn test_batch_processor() {
        let config = BatchWriterConfig {
            batch_size: 3,
            flush_interval: Duration::from_millis(100),
            max_buffer_size: 100,
            write_timeout: Duration::from_secs(1),
        };
        
        let writer = FileBatchWriter::new(
            "test_batch.jsonl".to_string(),
            "test_writer".to_string(),
        );
        
        let processor = BatchProcessor::new(config, writer);
        processor.start().await.unwrap();
        
        // 添加测试数据
        for i in 0..5 {
            let data = TestData {
                id: i,
                value: format!("test_{}", i),
            };
            processor.add(data).await.unwrap();
        }
        
        // 等待处理
        sleep(Duration::from_millis(200)).await;
        
        // 强制刷新
        processor.flush().await.unwrap();
        
        processor.stop().await;
    }
}