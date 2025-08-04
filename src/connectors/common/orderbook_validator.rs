//! 订单簿验证器
//! 实现WebSocket优化重构方案中的数据处理增强功能

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};
use log::{debug, warn, error};
use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
use crate::exchange_types::StandardOrderBook as OrderBook;

/// 订单簿档位
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookLevel {
    pub price: Decimal,
    pub quantity: Decimal,
}

/// 订单簿验证器
/// 负责验证订单簿数据的完整性和一致性
#[derive(Debug, Clone)]
pub struct OrderbookValidator {
    /// 配置参数
    config: OrderbookValidatorConfig,
    /// 验证统计信息
    stats: ValidationStats,
    /// 最后验证时间
    last_validation_time: Option<SystemTime>,
}

/// 订单簿验证器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderbookValidatorConfig {
    /// 是否启用严格验证
    pub strict_validation: bool,
    /// 最大价格偏差百分比
    pub max_price_deviation_percent: f64,
    /// 最小订单数量
    pub min_order_quantity: Decimal,
    /// 最大订单数量
    pub max_order_quantity: Decimal,
    /// 最大价差百分比
    pub max_spread_percent: f64,
    /// 最大档位数量
    pub max_levels: usize,
    /// 价格精度（小数位数）
    pub price_precision: u32,
    /// 数量精度（小数位数）
    pub quantity_precision: u32,
    /// 是否检查价格排序
    pub check_price_ordering: bool,
    /// 是否检查重复价格
    pub check_duplicate_prices: bool,
    /// 验证超时时间（毫秒）
    pub validation_timeout_ms: u64,
}

/// 验证统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationStats {
    /// 总验证次数
    pub total_validations: u64,
    /// 成功验证次数
    pub successful_validations: u64,
    /// 失败验证次数
    pub failed_validations: u64,
    /// 警告次数
    pub warning_count: u64,
    /// 平均验证时间（微秒）
    pub avg_validation_time_us: f64,
    /// 最后验证结果
    pub last_validation_result: Option<ValidationResult>,
}

/// 验证结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// 是否通过验证
    pub is_valid: bool,
    /// 验证耗时（微秒）
    pub validation_time_us: u64,
    /// 错误列表
    pub errors: Vec<ValidationError>,
    /// 警告列表
    pub warnings: Vec<ValidationWarning>,
    /// 订单簿统计信息
    pub orderbook_stats: OrderbookStats,
    /// 验证时间戳
    pub timestamp: SystemTime,
}

/// 验证错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// 错误类型
    pub error_type: ValidationErrorType,
    /// 错误消息
    pub message: String,
    /// 相关价格（如果适用）
    pub price: Option<Decimal>,
    /// 相关数量（如果适用）
    pub quantity: Option<Decimal>,
    /// 档位索引（如果适用）
    pub level_index: Option<usize>,
}

/// 验证警告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    /// 警告类型
    pub warning_type: ValidationWarningType,
    /// 警告消息
    pub message: String,
    /// 相关数据
    pub details: Option<String>,
}

/// 验证错误类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationErrorType {
    /// 价格排序错误
    PriceOrdering,
    /// 价格精度错误
    PricePrecision,
    /// 数量精度错误
    QuantityPrecision,
    /// 价格范围错误
    PriceRange,
    /// 数量范围错误
    QuantityRange,
    /// 价差过大
    SpreadTooLarge,
    /// 重复价格
    DuplicatePrice,
    /// 空订单簿
    EmptyOrderbook,
    /// 档位数量超限
    TooManyLevels,
    /// 数据格式错误
    DataFormat,
    /// 时间戳错误
    InvalidTimestamp,
}

/// 验证警告类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationWarningType {
    /// 价差较大
    LargeSpread,
    /// 数量异常
    UnusualQuantity,
    /// 价格跳跃
    PriceGap,
    /// 档位数量少
    FewLevels,
    /// 数据延迟
    DataDelay,
    /// 精度不一致
    InconsistentPrecision,
}

/// 订单簿统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderbookStats {
    /// 买单档位数量
    pub bid_levels: usize,
    /// 卖单档位数量
    pub ask_levels: usize,
    /// 最佳买价
    pub best_bid: Option<Decimal>,
    /// 最佳卖价
    pub best_ask: Option<Decimal>,
    /// 价差
    pub spread: Option<Decimal>,
    /// 价差百分比
    pub spread_percent: Option<f64>,
    /// 总买单量
    pub total_bid_quantity: Decimal,
    /// 总卖单量
    pub total_ask_quantity: Decimal,
    /// 中间价
    pub mid_price: Option<Decimal>,
    /// 加权平均价格
    pub weighted_avg_price: Option<Decimal>,
}

impl Default for OrderbookValidatorConfig {
    fn default() -> Self {
        Self {
            strict_validation: false,
            max_price_deviation_percent: 10.0,
            min_order_quantity: Decimal::new(1, 8), // 0.00000001
            max_order_quantity: Decimal::new(1000000, 0), // 1,000,000
            max_spread_percent: 5.0,
            max_levels: 1000,
            price_precision: 8,
            quantity_precision: 8,
            check_price_ordering: true,
            check_duplicate_prices: true,
            validation_timeout_ms: 100,
        }
    }
}

impl Default for ValidationStats {
    fn default() -> Self {
        Self {
            total_validations: 0,
            successful_validations: 0,
            failed_validations: 0,
            warning_count: 0,
            avg_validation_time_us: 0.0,
            last_validation_result: None,
        }
    }
}

impl OrderbookValidator {
    /// 创建新的订单簿验证器
    pub fn new(config: OrderbookValidatorConfig) -> Self {
        Self {
            config,
            stats: ValidationStats::default(),
            last_validation_time: None,
        }
    }

    /// 使用默认配置创建验证器
    pub fn with_default_config() -> Self {
        Self::new(OrderbookValidatorConfig::default())
    }

    /// 验证订单簿
    pub fn validate_orderbook(&mut self, orderbook: &OrderBook) -> ValidationResult {
        let start_time = SystemTime::now();
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        
        // 更新统计信息
        self.stats.total_validations += 1;
        self.last_validation_time = Some(start_time);
        
        // 检查超时
        let timeout = Duration::from_millis(self.config.validation_timeout_ms);
        
        // 基础验证
        self.validate_basic_structure(orderbook, &mut errors, &mut warnings);
        
        if start_time.elapsed().unwrap_or(Duration::from_secs(0)) > timeout {
            errors.push(ValidationError {
                error_type: ValidationErrorType::DataFormat,
                message: "验证超时".to_string(),
                price: None,
                quantity: None,
                level_index: None,
            });
        }
        
        // 价格验证
        if !errors.is_empty() && self.config.strict_validation {
            // 严格模式下，如果有基础错误就不继续验证
        } else {
            self.validate_prices(orderbook, &mut errors, &mut warnings);
            
            if start_time.elapsed().unwrap_or(Duration::from_secs(0)) <= timeout {
                self.validate_quantities(orderbook, &mut errors, &mut warnings);
            }
            
            if start_time.elapsed().unwrap_or(Duration::from_secs(0)) <= timeout {
                self.validate_spread(orderbook, &mut errors, &mut warnings);
            }
        }
        
        // 计算统计信息
        let orderbook_stats = self.calculate_orderbook_stats(orderbook);
        
        let validation_time_us = start_time.elapsed().unwrap_or(Duration::from_secs(0)).as_micros() as u64;
        let is_valid = errors.is_empty();
        
        // 更新统计信息
        if is_valid {
            self.stats.successful_validations += 1;
        } else {
            self.stats.failed_validations += 1;
        }
        
        self.stats.warning_count += warnings.len() as u64;
        
        // 更新平均验证时间
        let total_time = self.stats.avg_validation_time_us * (self.stats.total_validations - 1) as f64;
        self.stats.avg_validation_time_us = (total_time + validation_time_us as f64) / self.stats.total_validations as f64;
        
        let result = ValidationResult {
            is_valid,
            validation_time_us,
            errors,
            warnings,
            orderbook_stats,
            timestamp: start_time,
        };
        
        self.stats.last_validation_result = Some(result.clone());
        
        debug!("[OrderbookValidator] 验证完成: valid={}, errors={}, warnings={}, time={}μs",
               is_valid, result.errors.len(), result.warnings.len(), validation_time_us);
        
        result
    }

    /// 验证基础结构
    fn validate_basic_structure(
        &self,
        orderbook: &OrderBook,
        errors: &mut Vec<ValidationError>,
        warnings: &mut Vec<ValidationWarning>,
    ) {
        // 检查是否为空
        if orderbook.depth_bids.is_empty() && orderbook.depth_asks.is_empty() {
            errors.push(ValidationError {
                error_type: ValidationErrorType::EmptyOrderbook,
                message: "订单簿为空".to_string(),
                price: None,
                quantity: None,
                level_index: None,
            });
            return;
        }
        
        // 检查档位数量
        let total_levels = orderbook.depth_bids.len() + orderbook.depth_asks.len();
        if total_levels > self.config.max_levels {
            errors.push(ValidationError {
                error_type: ValidationErrorType::TooManyLevels,
                message: format!("档位数量过多: {}, 最大允许: {}", total_levels, self.config.max_levels),
                price: None,
                quantity: None,
                level_index: None,
            });
        }
        
        // 检查档位数量是否过少
        if orderbook.depth_bids.len() < 5 || orderbook.depth_asks.len() < 5 {
            warnings.push(ValidationWarning {
                warning_type: ValidationWarningType::FewLevels,
                message: format!("档位数量较少: bids={}, asks={}", orderbook.depth_bids.len(), orderbook.depth_asks.len()),
                details: None,
            });
        }
        
        // 检查时间戳
        if orderbook.timestamp > 0 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            
            let delay = now.saturating_sub(orderbook.timestamp as u64);
            if delay > 5000 { // 5秒延迟
                warnings.push(ValidationWarning {
                    warning_type: ValidationWarningType::DataDelay,
                    message: format!("数据延迟: {}ms", delay),
                    details: Some(format!("timestamp: {}, now: {}", orderbook.timestamp, now)),
                });
            }
        }
    }

    /// 验证价格
    fn validate_prices(
        &self,
        orderbook: &OrderBook,
        errors: &mut Vec<ValidationError>,
        warnings: &mut Vec<ValidationWarning>,
    ) {
        // 验证买单价格排序（从高到低）
        if self.config.check_price_ordering {
            for (i, level) in orderbook.depth_bids.iter().enumerate() {
                if i > 0 {
                    let prev_price = orderbook.depth_bids[i - 1].0;
                    if level.0 >= prev_price {
                        errors.push(ValidationError {
                            error_type: ValidationErrorType::PriceOrdering,
                            message: format!("买单价格排序错误: 第{}档价格{} >= 第{}档价格{}", 
                                           i + 1, level.0, i, prev_price),
                            price: Some(Decimal::try_from(level.0).unwrap_or_default()),
                            quantity: None,
                            level_index: Some(i),
                        });
                    }
                }
                
                // 检查价格精度
                let price_decimal = Decimal::try_from(level.0).unwrap_or_default();
                if !self.check_price_precision(price_decimal) {
                    errors.push(ValidationError {
                        error_type: ValidationErrorType::PricePrecision,
                        message: format!("买单价格精度错误: {}, 期望精度: {}", 
                                       price_decimal, self.config.price_precision),
                        price: Some(price_decimal),
                        quantity: None,
                        level_index: Some(i),
                    });
                }
            }
        }
        
        // 验证卖单价格排序（从低到高）
        if self.config.check_price_ordering {
            for (i, level) in orderbook.depth_asks.iter().enumerate() {
                if i > 0 {
                    let prev_price = orderbook.depth_asks[i - 1].0;
                    if level.0 <= prev_price {
                        errors.push(ValidationError {
                            error_type: ValidationErrorType::PriceOrdering,
                            message: format!("卖单价格排序错误: 第{}档价格{} <= 第{}档价格{}", 
                                           i + 1, level.0, i, prev_price),
                            price: Some(Decimal::try_from(level.0).unwrap_or_default()),
                            quantity: None,
                            level_index: Some(i),
                        });
                    }
                }
                
                // 检查价格精度
                let price_decimal = Decimal::try_from(level.0).unwrap_or_default();
                if !self.check_price_precision(price_decimal) {
                    errors.push(ValidationError {
                        error_type: ValidationErrorType::PricePrecision,
                        message: format!("卖单价格精度错误: {}, 期望精度: {}", 
                                       price_decimal, self.config.price_precision),
                        price: Some(price_decimal),
                        quantity: None,
                        level_index: Some(i),
                    });
                }
            }
        }
        
        // 检查重复价格
        if self.config.check_duplicate_prices {
            self.check_duplicate_prices_f64(&orderbook.depth_bids, "买单", errors);
            self.check_duplicate_prices_f64(&orderbook.depth_asks, "卖单", errors);
        }
    }

    /// 验证数量
    fn validate_quantities(
        &self,
        orderbook: &OrderBook,
        errors: &mut Vec<ValidationError>,
        warnings: &mut Vec<ValidationWarning>,
    ) {
        // 验证买单数量
        for (i, level) in orderbook.depth_bids.iter().enumerate() {
            let quantity_decimal = Decimal::try_from(level.1).unwrap_or_default();
            let price_decimal = Decimal::try_from(level.0).unwrap_or_default();
            
            if quantity_decimal < self.config.min_order_quantity {
                errors.push(ValidationError {
                    error_type: ValidationErrorType::QuantityRange,
                    message: format!("买单数量过小: {}, 最小值: {}", 
                                   quantity_decimal, self.config.min_order_quantity),
                    price: Some(price_decimal),
                    quantity: Some(quantity_decimal),
                    level_index: Some(i),
                });
            }
            
            if quantity_decimal > self.config.max_order_quantity {
                warnings.push(ValidationWarning {
                    warning_type: ValidationWarningType::UnusualQuantity,
                    message: format!("买单数量异常大: {}, 最大值: {}", 
                                   quantity_decimal, self.config.max_order_quantity),
                    details: Some(format!("price: {}, level: {}", price_decimal, i)),
                });
            }
            
            // 检查数量精度
            if !self.check_quantity_precision(quantity_decimal) {
                errors.push(ValidationError {
                    error_type: ValidationErrorType::QuantityPrecision,
                    message: format!("买单数量精度错误: {}, 期望精度: {}", 
                                   quantity_decimal, self.config.quantity_precision),
                    price: Some(price_decimal),
                    quantity: Some(quantity_decimal),
                    level_index: Some(i),
                });
            }
        }
        
        // 验证卖单数量
        for (i, level) in orderbook.depth_asks.iter().enumerate() {
            let quantity_decimal = Decimal::try_from(level.1).unwrap_or_default();
            let price_decimal = Decimal::try_from(level.0).unwrap_or_default();
            
            if quantity_decimal < self.config.min_order_quantity {
                errors.push(ValidationError {
                    error_type: ValidationErrorType::QuantityRange,
                    message: format!("卖单数量过小: {}, 最小值: {}", 
                                   quantity_decimal, self.config.min_order_quantity),
                    price: Some(price_decimal),
                    quantity: Some(quantity_decimal),
                    level_index: Some(i),
                });
            }
            
            if quantity_decimal > self.config.max_order_quantity {
                warnings.push(ValidationWarning {
                    warning_type: ValidationWarningType::UnusualQuantity,
                    message: format!("卖单数量异常大: {}, 最大值: {}", 
                                   quantity_decimal, self.config.max_order_quantity),
                    details: Some(format!("price: {}, level: {}", price_decimal, i)),
                });
            }
            
            // 检查数量精度
            if !self.check_quantity_precision(quantity_decimal) {
                errors.push(ValidationError {
                    error_type: ValidationErrorType::QuantityPrecision,
                    message: format!("卖单数量精度错误: {}, 期望精度: {}", 
                                   quantity_decimal, self.config.quantity_precision),
                    price: Some(price_decimal),
                    quantity: Some(quantity_decimal),
                    level_index: Some(i),
                });
            }
        }
    }

    /// 验证价差
    fn validate_spread(
        &self,
        orderbook: &OrderBook,
        errors: &mut Vec<ValidationError>,
        warnings: &mut Vec<ValidationWarning>,
    ) {
        if orderbook.depth_bids.is_empty() || orderbook.depth_asks.is_empty() {
            return;
        }
        
        let best_bid = Decimal::try_from(orderbook.depth_bids[0].0).unwrap_or_default();
        let best_ask = Decimal::try_from(orderbook.depth_asks[0].0).unwrap_or_default();
        
        if best_bid >= best_ask {
            errors.push(ValidationError {
                error_type: ValidationErrorType::PriceRange,
                message: format!("价格交叉: 最佳买价{} >= 最佳卖价{}", best_bid, best_ask),
                price: None,
                quantity: None,
                level_index: None,
            });
            return;
        }
        
        let spread = best_ask - best_bid;
        let mid_price = (best_bid + best_ask) / Decimal::from(2);
        let spread_percent = if mid_price > Decimal::ZERO {
            f64::try_from(spread / mid_price * Decimal::from(100)).unwrap_or(0.0)
        } else {
            0.0
        };
        
        if spread_percent > self.config.max_spread_percent {
            errors.push(ValidationError {
                error_type: ValidationErrorType::SpreadTooLarge,
                message: format!("价差过大: {:.4}%, 最大允许: {:.2}%", 
                               spread_percent, self.config.max_spread_percent),
                price: None,
                quantity: None,
                level_index: None,
            });
        } else if spread_percent > self.config.max_spread_percent * 0.8 {
            warnings.push(ValidationWarning {
                warning_type: ValidationWarningType::LargeSpread,
                message: format!("价差较大: {:.4}%", spread_percent),
                details: Some(format!("best_bid: {}, best_ask: {}, spread: {}", 
                                    best_bid, best_ask, spread)),
            });
        }
    }

    /// 检查价格精度
    fn check_price_precision(&self, price: Decimal) -> bool {
        let scale = price.scale();
        scale <= self.config.price_precision
    }

    /// 检查数量精度
    fn check_quantity_precision(&self, quantity: Decimal) -> bool {
        let scale = quantity.scale();
        scale <= self.config.quantity_precision
    }

    /// 检查重复价格
    fn check_duplicate_prices(
        &self,
        levels: &[OrderBookLevel],
        side: &str,
        errors: &mut Vec<ValidationError>,
    ) {
        let mut seen_prices = std::collections::HashSet::new();
        
        for (i, level) in levels.iter().enumerate() {
            if !seen_prices.insert(level.price) {
                errors.push(ValidationError {
                    error_type: ValidationErrorType::DuplicatePrice,
                    message: format!("{}重复价格: {}", side, level.price),
                    price: Some(level.price),
                    quantity: None,
                    level_index: Some(i),
                });
            }
        }
    }

    /// 检查重复价格（f64版本）
    fn check_duplicate_prices_f64(
        &self,
        levels: &[(f64, f64)],
        side: &str,
        errors: &mut Vec<ValidationError>,
    ) {
        let mut seen_prices = std::collections::HashSet::new();
        
        for (i, level) in levels.iter().enumerate() {
            let price_decimal = Decimal::try_from(level.0).unwrap_or_default();
            if !seen_prices.insert(price_decimal) {
                errors.push(ValidationError {
                    error_type: ValidationErrorType::DuplicatePrice,
                    message: format!("{}重复价格: {}", side, price_decimal),
                    price: Some(price_decimal),
                    quantity: None,
                    level_index: Some(i),
                });
            }
        }
    }

    /// 计算订单簿统计信息
    fn calculate_orderbook_stats(&self, orderbook: &OrderBook) -> OrderbookStats {
        let best_bid = orderbook.depth_bids.first().map(|level| Decimal::try_from(level.0).unwrap_or_default());
        let best_ask = orderbook.depth_asks.first().map(|level| Decimal::try_from(level.0).unwrap_or_default());
        
        let spread = match (best_bid, best_ask) {
            (Some(bid), Some(ask)) if ask > bid => Some(ask - bid),
            _ => None,
        };
        
        let spread_percent = match (spread, best_bid, best_ask) {
            (Some(s), Some(bid), Some(ask)) => {
                let mid_price = (bid + ask) / Decimal::from(2);
                if mid_price > Decimal::ZERO {
                    Some(f64::try_from(s / mid_price * Decimal::from(100)).unwrap_or(0.0))
                } else {
                    None
                }
            }
            _ => None,
        };
        
        let mid_price = match (best_bid, best_ask) {
            (Some(bid), Some(ask)) => Some((bid + ask) / Decimal::from(2)),
            _ => None,
        };
        
        let total_bid_quantity = orderbook.depth_bids.iter()
            .map(|level| Decimal::try_from(level.1).unwrap_or_default())
            .sum();
        
        let total_ask_quantity = orderbook.depth_asks.iter()
            .map(|level| Decimal::try_from(level.1).unwrap_or_default())
            .sum();
        
        // 计算加权平均价格（简化版本）
        let weighted_avg_price = if !orderbook.depth_bids.is_empty() && !orderbook.depth_asks.is_empty() {
            let bid_value: Decimal = orderbook.depth_bids.iter()
                .map(|level| Decimal::try_from(level.0).unwrap_or_default() * Decimal::try_from(level.1).unwrap_or_default())
                .sum();
            let ask_value: Decimal = orderbook.depth_asks.iter()
                .map(|level| Decimal::try_from(level.0).unwrap_or_default() * Decimal::try_from(level.1).unwrap_or_default())
                .sum();
            
            let total_quantity = total_bid_quantity + total_ask_quantity;
            if total_quantity > Decimal::ZERO {
                Some((bid_value + ask_value) / total_quantity)
            } else {
                None
            }
        } else {
            None
        };
        
        OrderbookStats {
            bid_levels: orderbook.depth_bids.len(),
            ask_levels: orderbook.depth_asks.len(),
            best_bid,
            best_ask,
            spread,
            spread_percent,
            total_bid_quantity,
            total_ask_quantity,
            mid_price,
            weighted_avg_price,
        }
    }

    /// 快速验证（仅基础检查）
    pub fn quick_validate(&mut self, orderbook: &OrderBook) -> bool {
        // 快速检查基础结构
        if orderbook.depth_bids.is_empty() && orderbook.depth_asks.is_empty() {
            return false;
        }
        
        // 快速检查价格交叉
        if !orderbook.depth_bids.is_empty() && !orderbook.depth_asks.is_empty() {
            let best_bid = orderbook.depth_bids[0].0;
            let best_ask = orderbook.depth_asks[0].0;
            if best_bid >= best_ask {
                return false;
            }
        }
        
        true
    }

    /// 获取验证统计信息
    pub fn get_stats(&self) -> &ValidationStats {
        &self.stats
    }

    /// 重置统计信息
    pub fn reset_stats(&mut self) {
        self.stats = ValidationStats::default();
        debug!("[OrderbookValidator] 统计信息已重置");
    }

    /// 更新配置
    pub fn update_config(&mut self, new_config: OrderbookValidatorConfig) {
        self.config = new_config;
        debug!("[OrderbookValidator] 配置已更新");
    }

    /// 获取配置
    pub fn get_config(&self) -> &OrderbookValidatorConfig {
        &self.config
    }

    /// 生成验证报告
    pub fn generate_report(&self) -> ValidationReport {
        ValidationReport {
            config: self.config.clone(),
            stats: self.stats.clone(),
            last_validation_time: self.last_validation_time,
            success_rate: if self.stats.total_validations > 0 {
                self.stats.successful_validations as f64 / self.stats.total_validations as f64 * 100.0
            } else {
                0.0
            },
        }
    }
}

/// 验证报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub config: OrderbookValidatorConfig,
    pub stats: ValidationStats,
    pub last_validation_time: Option<SystemTime>,
    pub success_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_orderbook() -> OrderBook {
        OrderBook {
            symbol: "BTCUSDT".to_string(),
            exchange: crate::exchange_types::Exchange::Binance,
            best_bid: 50000.0,
            best_ask: 50001.0,
            depth_bids: vec![
                (50000.0, 1.0),
                (49999.0, 2.0),
                (49998.0, 3.0),
            ],
            depth_asks: vec![
                (50001.0, 1.0),
                (50002.0, 2.0),
                (50003.0, 3.0),
            ],
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64,
        }
    }

    #[test]
    fn test_validator_creation() {
        let validator = OrderbookValidator::with_default_config();
        let stats = validator.get_stats();
        
        assert_eq!(stats.total_validations, 0);
        assert_eq!(stats.successful_validations, 0);
        assert_eq!(stats.failed_validations, 0);
    }

    #[test]
    fn test_valid_orderbook() {
        let mut validator = OrderbookValidator::with_default_config();
        let orderbook = create_test_orderbook();
        
        let result = validator.validate_orderbook(&orderbook);
        
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
        assert_eq!(result.orderbook_stats.bid_levels, 3);
        assert_eq!(result.orderbook_stats.ask_levels, 3);
    }

    #[test]
    fn test_empty_orderbook() {
        let mut validator = OrderbookValidator::with_default_config();
        let orderbook = OrderBook {
            symbol: "BTCUSDT".to_string(),
            exchange: crate::exchange_types::Exchange::Binance,
            best_bid: 0.0,
            best_ask: 0.0,
            depth_bids: vec![],
            depth_asks: vec![],
            timestamp: 0,
        };
        
        let result = validator.validate_orderbook(&orderbook);
        
        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());
        assert_eq!(result.errors[0].error_type, ValidationErrorType::EmptyOrderbook);
    }

    #[test]
    fn test_price_ordering_error() {
        let mut validator = OrderbookValidator::with_default_config();
        let mut orderbook = create_test_orderbook();
        
        // 破坏买单价格排序
        orderbook.depth_bids[1].0 = 50001.0; // 应该小于第一档
        
        let result = validator.validate_orderbook(&orderbook);
        
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.error_type == ValidationErrorType::PriceOrdering));
    }

    #[test]
    fn test_price_cross_error() {
        let mut validator = OrderbookValidator::with_default_config();
        let mut orderbook = create_test_orderbook();
        
        // 制造价格交叉
        orderbook.depth_bids[0].0 = 50002.0; // 买价高于卖价
        
        let result = validator.validate_orderbook(&orderbook);
        
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.error_type == ValidationErrorType::PriceRange));
    }

    #[test]
    fn test_quick_validate() {
        let mut validator = OrderbookValidator::with_default_config();
        let orderbook = create_test_orderbook();
        
        assert!(validator.quick_validate(&orderbook));
        
        let empty_orderbook = OrderBook {
            symbol: "BTCUSDT".to_string(),
            exchange: crate::exchange_types::Exchange::Binance,
            best_bid: 0.0,
            best_ask: 0.0,
            depth_bids: vec![],
            depth_asks: vec![],
            timestamp: 0,
        };
        
        assert!(!validator.quick_validate(&empty_orderbook));
    }

    #[test]
    fn test_statistics_update() {
        let mut validator = OrderbookValidator::with_default_config();
        let orderbook = create_test_orderbook();
        
        // 第一次验证
        let result1 = validator.validate_orderbook(&orderbook);
        assert!(result1.is_valid);
        
        let stats = validator.get_stats();
        assert_eq!(stats.total_validations, 1);
        assert_eq!(stats.successful_validations, 1);
        assert_eq!(stats.failed_validations, 0);
        
        // 第二次验证（失败的）
        let empty_orderbook = OrderBook {
            symbol: "BTCUSDT".to_string(),
            exchange: crate::exchange_types::Exchange::Binance,
            best_bid: 0.0,
            best_ask: 0.0,
            depth_bids: vec![],
            depth_asks: vec![],
            timestamp: 0,
        };
        
        let result2 = validator.validate_orderbook(&empty_orderbook);
        assert!(!result2.is_valid);
        
        let stats = validator.get_stats();
        assert_eq!(stats.total_validations, 2);
        assert_eq!(stats.successful_validations, 1);
        assert_eq!(stats.failed_validations, 1);
    }

    #[test]
    fn test_generate_report() {
        let mut validator = OrderbookValidator::with_default_config();
        let orderbook = create_test_orderbook();
        
        validator.validate_orderbook(&orderbook);
        
        let report = validator.generate_report();
        assert_eq!(report.success_rate, 100.0);
        assert_eq!(report.stats.total_validations, 1);
    }
}