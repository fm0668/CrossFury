//! Binance期货风险管理模块
//! 
//! 实现仓位限制检查、保证金充足性验证、价格偏离保护和紧急停止机制

use crate::core::AppError;
use crate::connectors::binance::futures::websocket::{FuturesPosition, FuturesBalance};
use crate::types::OrderRequest;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use log::{info, error};
use chrono::{DateTime, Utc};

// 定义Result类型别名
pub type Result<T> = std::result::Result<T, AppError>;

/// 仓位限制检查器
#[derive(Debug, Clone)]
pub struct PositionLimitChecker {
    /// 每个交易对的最大持仓量
    max_position_size: HashMap<String, f64>,
    /// 每个交易对的最大单笔订单量
    max_order_size: HashMap<String, f64>,
    /// 持仓集中度限制 (0.0-1.0)
    concentration_limit: f64,
    /// 全局最大持仓价值 (USDT)
    max_total_position_value: f64,
}

impl PositionLimitChecker {
    /// 创建新的仓位限制检查器
    pub fn new() -> Self {
        Self {
            max_position_size: HashMap::new(),
            max_order_size: HashMap::new(),
            concentration_limit: 0.3, // 默认30%集中度限制
            max_total_position_value: 100000.0, // 默认10万USDT
        }
    }
    
    /// 设置交易对的最大持仓量
    pub fn set_max_position_size(&mut self, symbol: &str, max_size: f64) {
        self.max_position_size.insert(symbol.to_string(), max_size);
    }
    
    /// 设置交易对的最大单笔订单量
    pub fn set_max_order_size(&mut self, symbol: &str, max_size: f64) {
        self.max_order_size.insert(symbol.to_string(), max_size);
    }
    
    /// 设置持仓集中度限制
    pub fn set_concentration_limit(&mut self, limit: f64) {
        self.concentration_limit = limit.clamp(0.0, 1.0);
    }
    
    /// 设置全局最大持仓价值
    pub fn set_max_total_position_value(&mut self, value: f64) {
        self.max_total_position_value = value;
    }
    
    /// 检查持仓限制
    pub fn check_position_limit(&self, symbol: &str, new_position_size: f64, current_positions: &[FuturesPosition]) -> Result<()> {
        // 检查单个交易对持仓限制
        if let Some(&max_size) = self.max_position_size.get(symbol) {
            if new_position_size.abs() > max_size {
                return Err(AppError::RiskError(format!(
                    "持仓量{new_position_size}超过{symbol}的最大限制{max_size}"
                )));
            }
        }
        
        // 检查持仓集中度
        self.check_concentration(symbol, new_position_size, current_positions)?;
        
        // 检查全局持仓价值
        self.check_total_position_value(symbol, new_position_size, current_positions)?;
        
        Ok(())
    }
    
    /// 检查订单限制
    pub fn check_order_limit(&self, symbol: &str, order_size: f64) -> Result<()> {
        if let Some(&max_size) = self.max_order_size.get(symbol) {
            if order_size.abs() > max_size {
                return Err(AppError::RiskError(format!(
                    "订单量{order_size}超过{symbol}的最大限制{max_size}"
                )));
            }
        }
        Ok(())
    }
    
    /// 检查持仓集中度
    pub fn check_concentration(&self, symbol: &str, new_position_size: f64, current_positions: &[FuturesPosition]) -> Result<()> {
        let total_value: f64 = current_positions.iter()
            .map(|pos| pos.notional.abs())
            .sum();
            
        if total_value == 0.0 {
            return Ok(()); // 没有持仓时不检查集中度
        }
        
        // 计算新持仓后的集中度
        let symbol_current_value = current_positions.iter()
            .find(|pos| pos.symbol == symbol)
            .map(|pos| pos.notional.abs())
            .unwrap_or(0.0);
            
        // 估算新持仓价值 (使用标记价格)
        let new_position_value = current_positions.iter()
            .find(|pos| pos.symbol == symbol)
            .map(|pos| new_position_size.abs() * pos.mark_price)
            .unwrap_or(0.0);
            
        let new_total_value = total_value - symbol_current_value + new_position_value;
        let concentration = if new_total_value > 0.0 {
            new_position_value / new_total_value
        } else {
            0.0
        };
        
        if concentration > self.concentration_limit {
            return Err(AppError::RiskError(format!(
                "{}持仓集中度{:.2}%超过限制{:.2}%", 
                symbol, concentration * 100.0, self.concentration_limit * 100.0
            )));
        }
        
        Ok(())
    }
    
    /// 检查全局持仓价值
    fn check_total_position_value(&self, symbol: &str, new_position_size: f64, current_positions: &[FuturesPosition]) -> Result<()> {
        let current_total_value: f64 = current_positions.iter()
            .map(|pos| pos.notional.abs())
            .sum();
            
        // 估算新持仓价值
        let new_position_value = current_positions.iter()
            .find(|pos| pos.symbol == symbol)
            .map(|pos| new_position_size.abs() * pos.mark_price)
            .unwrap_or(0.0);
            
        let symbol_current_value = current_positions.iter()
            .find(|pos| pos.symbol == symbol)
            .map(|pos| pos.notional.abs())
            .unwrap_or(0.0);
            
        let new_total_value = current_total_value - symbol_current_value + new_position_value;
        
        if new_total_value > self.max_total_position_value {
            return Err(AppError::RiskError(format!(
                "总持仓价值{:.2}超过最大限制{:.2}", 
                new_total_value, self.max_total_position_value
            )));
        }
        
        Ok(())
    }
}

impl Default for PositionLimitChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// 保证金检查器
#[derive(Debug, Clone)]
pub struct MarginChecker {
    /// 保证金率阈值
    margin_ratio_threshold: f64,
    /// 维持保证金率
    maintenance_margin_ratio: f64,
    /// 初始保证金率
    initial_margin_ratio: f64,
}

impl MarginChecker {
    /// 创建新的保证金检查器
    pub fn new() -> Self {
        Self {
            margin_ratio_threshold: 0.8, // 80%保证金率阈值
            maintenance_margin_ratio: 0.05, // 5%维持保证金率
            initial_margin_ratio: 0.1, // 10%初始保证金率
        }
    }
    
    /// 设置保证金率阈值
    pub fn set_margin_ratio_threshold(&mut self, threshold: f64) {
        self.margin_ratio_threshold = threshold.clamp(0.0, 1.0);
    }
    
    /// 设置维持保证金率
    pub fn set_maintenance_margin_ratio(&mut self, ratio: f64) {
        self.maintenance_margin_ratio = ratio.clamp(0.0, 1.0);
    }
    
    /// 设置初始保证金率
    pub fn set_initial_margin_ratio(&mut self, ratio: f64) {
        self.initial_margin_ratio = ratio.clamp(0.0, 1.0);
    }
    
    /// 检查保证金充足性
    pub fn check_margin_sufficiency(&self, balances: &[FuturesBalance], positions: &[FuturesPosition], order: &OrderRequest) -> Result<()> {
        let required_margin = self.calculate_required_margin(order, positions)?;
        let available_margin = self.calculate_available_margin(balances);
        
        if available_margin < required_margin {
            return Err(AppError::RiskError(format!(
                "保证金不足: 需要{required_margin:.2}, 可用{available_margin:.2}"
            )));
        }
        
        // 检查下单后的保证金率
        let total_margin = balances.iter()
            .map(|b| b.margin_balance)
            .sum::<f64>();
            
        let total_position_value: f64 = positions.iter()
            .map(|pos| pos.notional.abs())
            .sum();
            
        // 估算新订单的持仓价值
        let order_value = order.quantity * order.price.unwrap_or(0.0);
        let new_total_position_value = total_position_value + order_value;
        
        let margin_ratio = if new_total_position_value > 0.0 {
            total_margin / new_total_position_value
        } else {
            1.0
        };
        
        if margin_ratio < self.margin_ratio_threshold {
            return Err(AppError::RiskError(format!(
                "保证金率{:.2}%低于阈值{:.2}%", 
                margin_ratio * 100.0, self.margin_ratio_threshold * 100.0
            )));
        }
        
        Ok(())
    }
    
    /// 计算所需保证金
    pub fn calculate_required_margin(&self, order: &OrderRequest, positions: &[FuturesPosition]) -> Result<f64> {
        let order_value = order.quantity * order.price.unwrap_or(0.0);
        
        // 获取杠杆倍数 (从现有持仓中获取，如果没有则使用默认值)
        let leverage = positions.iter()
            .find(|pos| pos.symbol == order.symbol)
            .map(|pos| pos.leverage as f64)
            .unwrap_or(1.0);
            
        let required_margin = order_value / leverage * self.initial_margin_ratio;
        Ok(required_margin)
    }
    
    /// 计算可用保证金
    pub fn calculate_available_margin(&self, balances: &[FuturesBalance]) -> f64 {
        balances.iter()
            .map(|balance| balance.available_balance)
            .sum()
    }
    
    /// 计算保证金率
    pub fn calculate_margin_ratio(&self, balances: &[FuturesBalance], positions: &[FuturesPosition]) -> f64 {
        let total_margin: f64 = balances.iter()
            .map(|b| b.margin_balance)
            .sum();
            
        let total_position_value: f64 = positions.iter()
            .map(|pos| pos.notional.abs())
            .sum();
            
        if total_position_value > 0.0 {
            total_margin / total_position_value
        } else {
            1.0 // 没有持仓时保证金率为100%
        }
    }
}

impl Default for MarginChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// 价格保护器
#[derive(Debug, Clone)]
pub struct PriceProtection {
    /// 最大价格偏离百分比
    max_deviation_percent: f64,
    /// 参考价格缓存
    reference_prices: Arc<RwLock<HashMap<String, f64>>>,
    /// 价格更新时间
    price_update_time: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
    /// 价格有效期 (秒)
    price_validity_seconds: i64,
}

impl PriceProtection {
    /// 创建新的价格保护器
    pub fn new() -> Self {
        Self {
            max_deviation_percent: 5.0, // 默认5%偏离限制
            reference_prices: Arc::new(RwLock::new(HashMap::new())),
            price_update_time: Arc::new(RwLock::new(HashMap::new())),
            price_validity_seconds: 30, // 30秒价格有效期
        }
    }
    
    /// 设置最大价格偏离百分比
    pub fn set_max_deviation_percent(&mut self, percent: f64) {
        self.max_deviation_percent = percent.max(0.0);
    }
    
    /// 设置价格有效期
    pub fn set_price_validity_seconds(&mut self, seconds: i64) {
        self.price_validity_seconds = seconds.max(1);
    }
    
    /// 更新参考价格
    pub async fn update_reference_price(&self, symbol: &str, price: f64) {
        let mut prices = self.reference_prices.write().await;
        let mut times = self.price_update_time.write().await;
        
        prices.insert(symbol.to_string(), price);
        times.insert(symbol.to_string(), Utc::now());
    }
    
    /// 检查价格偏离
    pub async fn check_price_deviation(&self, symbol: &str, order_price: f64) -> Result<()> {
        let reference_price = self.get_reference_price(symbol).await?;
        
        let deviation = ((order_price - reference_price) / reference_price * 100.0).abs();
        
        if deviation > self.max_deviation_percent {
            return Err(AppError::RiskError(format!(
                "{}价格偏离{:.2}%超过限制{:.2}%, 订单价格: {}, 参考价格: {}", 
                symbol, deviation, self.max_deviation_percent, order_price, reference_price
            )));
        }
        
        Ok(())
    }
    
    /// 获取参考价格
    pub async fn get_reference_price(&self, symbol: &str) -> Result<f64> {
        let prices = self.reference_prices.read().await;
        let times = self.price_update_time.read().await;
        
        if let (Some(&price), Some(&update_time)) = (prices.get(symbol), times.get(symbol)) {
            let now = Utc::now();
            let age = (now - update_time).num_seconds();
            
            if age <= self.price_validity_seconds {
                Ok(price)
            } else {
                Err(AppError::RiskError(format!(
                    "{symbol}的参考价格已过期 ({age}秒前更新)"
                )))
            }
        } else {
            Err(AppError::RiskError(format!(
                "{symbol}没有可用的参考价格"
            )))
        }
    }
}

impl Default for PriceProtection {
    fn default() -> Self {
        Self::new()
    }
}

/// 紧急停止机制
#[derive(Debug, Clone)]
pub struct EmergencyStop {
    /// 是否处于紧急模式
    is_emergency_mode: Arc<RwLock<bool>>,
    /// 紧急停止原因
    emergency_reason: Arc<RwLock<Option<String>>>,
    /// 紧急停止时间
    emergency_time: Arc<RwLock<Option<DateTime<Utc>>>>,
    /// 紧急触发条件
    emergency_triggers: Vec<EmergencyTrigger>,
}

/// 紧急触发条件
#[derive(Debug, Clone)]
pub enum EmergencyTrigger {
    /// 保证金率过低
    LowMarginRatio(f64),
    /// 总亏损超过阈值
    ExcessiveLoss(f64),
    /// 连接断开时间过长
    ConnectionTimeout(i64),
    /// 手动触发
    Manual,
}

impl EmergencyStop {
    /// 创建新的紧急停止机制
    pub fn new() -> Self {
        Self {
            is_emergency_mode: Arc::new(RwLock::new(false)),
            emergency_reason: Arc::new(RwLock::new(None)),
            emergency_time: Arc::new(RwLock::new(None)),
            emergency_triggers: vec![
                EmergencyTrigger::LowMarginRatio(0.1), // 10%保证金率触发
                EmergencyTrigger::ExcessiveLoss(10000.0), // 1万USDT亏损触发
                EmergencyTrigger::ConnectionTimeout(300), // 5分钟连接超时触发
            ],
        }
    }
    
    /// 添加紧急触发条件
    pub fn add_trigger(&mut self, trigger: EmergencyTrigger) {
        self.emergency_triggers.push(trigger);
    }
    
    /// 触发紧急停止
    pub async fn trigger_emergency_stop(&self, reason: &str) -> Result<()> {
        let mut is_emergency = self.is_emergency_mode.write().await;
        let mut emergency_reason = self.emergency_reason.write().await;
        let mut emergency_time = self.emergency_time.write().await;
        
        *is_emergency = true;
        *emergency_reason = Some(reason.to_string());
        *emergency_time = Some(Utc::now());
        
        error!("紧急停止已触发: {reason}");
        
        Ok(())
    }
    
    /// 解除紧急停止
    pub async fn clear_emergency_stop(&self) -> Result<()> {
        let mut is_emergency = self.is_emergency_mode.write().await;
        let mut emergency_reason = self.emergency_reason.write().await;
        let mut emergency_time = self.emergency_time.write().await;
        
        *is_emergency = false;
        *emergency_reason = None;
        *emergency_time = None;
        
        info!("紧急停止已解除");
        
        Ok(())
    }
    
    /// 检查是否处于紧急模式
    pub async fn is_emergency_mode(&self) -> bool {
        *self.is_emergency_mode.read().await
    }
    
    /// 获取紧急停止信息
    pub async fn get_emergency_info(&self) -> Option<(String, DateTime<Utc>)> {
        let reason = self.emergency_reason.read().await;
        let time = self.emergency_time.read().await;
        
        if let (Some(r), Some(t)) = (reason.as_ref(), time.as_ref()) {
            Some((r.clone(), *t))
        } else {
            None
        }
    }
    
    /// 检查紧急触发条件
    pub async fn check_emergency_triggers(&self, balances: &[FuturesBalance], positions: &[FuturesPosition], connection_lost_seconds: i64) -> Result<()> {
        if self.is_emergency_mode().await {
            return Ok(()); // 已经处于紧急模式
        }
        
        for trigger in &self.emergency_triggers {
            match trigger {
                EmergencyTrigger::LowMarginRatio(threshold) => {
                    let margin_checker = MarginChecker::new();
                    let margin_ratio = margin_checker.calculate_margin_ratio(balances, positions);
                    if margin_ratio < *threshold {
                        self.trigger_emergency_stop(&format!("保证金率{:.2}%低于紧急阈值{:.2}%", margin_ratio * 100.0, threshold * 100.0)).await?;
                        return Ok(());
                    }
                }
                EmergencyTrigger::ExcessiveLoss(threshold) => {
                    let total_pnl: f64 = positions.iter()
                        .map(|pos| pos.unrealized_profit)
                        .sum();
                    if total_pnl < -threshold {
                        self.trigger_emergency_stop(&format!("总亏损{:.2}超过紧急阈值{:.2}", -total_pnl, threshold)).await?;
                        return Ok(());
                    }
                }
                EmergencyTrigger::ConnectionTimeout(threshold) => {
                    if connection_lost_seconds > *threshold {
                        self.trigger_emergency_stop(&format!("连接断开{connection_lost_seconds}秒超过紧急阈值{threshold}秒")).await?;
                        return Ok(());
                    }
                }
                EmergencyTrigger::Manual => {
                    // 手动触发由外部调用
                }
            }
        }
        
        Ok(())
    }
}

impl Default for EmergencyStop {
    fn default() -> Self {
        Self::new()
    }
}

/// 综合风险管理器
#[derive(Debug)]
pub struct RiskManager {
    /// 仓位限制检查器
    position_checker: PositionLimitChecker,
    /// 保证金检查器
    margin_checker: MarginChecker,
    /// 价格保护器
    price_protection: PriceProtection,
    /// 紧急停止机制
    emergency_stop: EmergencyStop,
    /// 是否启用风险检查
    enabled: bool,
}

impl RiskManager {
    /// 创建新的风险管理器
    pub fn new() -> Self {
        Self {
            position_checker: PositionLimitChecker::new(),
            margin_checker: MarginChecker::new(),
            price_protection: PriceProtection::new(),
            emergency_stop: EmergencyStop::new(),
            enabled: true,
        }
    }
    
    /// 启用/禁用风险检查
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    
    /// 获取仓位限制检查器的可变引用
    pub fn position_checker_mut(&mut self) -> &mut PositionLimitChecker {
        &mut self.position_checker
    }
    
    /// 获取保证金检查器的可变引用
    pub fn margin_checker_mut(&mut self) -> &mut MarginChecker {
        &mut self.margin_checker
    }
    
    /// 获取价格保护器的可变引用
    pub fn price_protection_mut(&mut self) -> &mut PriceProtection {
        &mut self.price_protection
    }
    
    /// 获取紧急停止机制的可变引用
    pub fn emergency_stop_mut(&mut self) -> &mut EmergencyStop {
        &mut self.emergency_stop
    }
    
    /// 综合风险检查 (下单前)
    pub async fn check_order_risk(&self, order: &OrderRequest, balances: &[FuturesBalance], positions: &[FuturesPosition]) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        
        // 检查紧急停止状态
        if self.emergency_stop.is_emergency_mode().await {
            if let Some((reason, time)) = self.emergency_stop.get_emergency_info().await {
                return Err(AppError::RiskError(format!(
                    "系统处于紧急停止状态: {} (触发时间: {})", 
                    reason, time.format("%Y-%m-%d %H:%M:%S UTC")
                )));
            }
        }
        
        // 检查订单限制
        self.position_checker.check_order_limit(&order.symbol, order.quantity)?;
        
        // 检查价格偏离 (如果是限价单)
        if let Some(price) = order.price {
            self.price_protection.check_price_deviation(&order.symbol, price).await?;
        }
        
        // 检查保证金充足性
        self.margin_checker.check_margin_sufficiency(balances, positions, order)?;
        
        // 计算新持仓并检查持仓限制
        let new_position_size = self.calculate_new_position_size(order, positions);
        self.position_checker.check_position_limit(&order.symbol, new_position_size, positions)?;
        
        Ok(())
    }
    
    /// 更新市场价格 (用于价格偏离检查)
    pub async fn update_market_price(&self, symbol: &str, price: f64) {
        self.price_protection.update_reference_price(symbol, price).await;
    }
    
    /// 检查紧急触发条件
    pub async fn check_emergency_conditions(&self, balances: &[FuturesBalance], positions: &[FuturesPosition], connection_lost_seconds: i64) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        
        self.emergency_stop.check_emergency_triggers(balances, positions, connection_lost_seconds).await
    }
    
    /// 手动触发紧急停止
    pub async fn trigger_manual_emergency_stop(&self, reason: &str) -> Result<()> {
        self.emergency_stop.trigger_emergency_stop(reason).await
    }
    
    /// 解除紧急停止
    pub async fn clear_emergency_stop(&self) -> Result<()> {
        self.emergency_stop.clear_emergency_stop().await
    }
    
    /// 计算新的持仓大小
    fn calculate_new_position_size(&self, order: &OrderRequest, positions: &[FuturesPosition]) -> f64 {
        let current_position = positions.iter()
            .find(|pos| pos.symbol == order.symbol)
            .map(|pos| pos.position_amt)
            .unwrap_or(0.0);
            
        let order_quantity = match order.side.as_str() {
            "BUY" => order.quantity,
            "SELL" => -order.quantity,
            _ => 0.0,
        };
        
        current_position + order_quantity
    }
}

impl Default for RiskManager {
    fn default() -> Self {
        Self::new()
    }
}