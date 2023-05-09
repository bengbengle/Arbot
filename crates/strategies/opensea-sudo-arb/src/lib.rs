//! A strategy implementing atomic, cross-market NFT arbitrage between
//! Seaport and Sudoswap. At a high level, we listen to a stream of new seaport orders,
//! and compute whether we can atomically fulfill the order and sell the NFT into a
//! sudoswap pool while making a profit.

/// This module contains constants used by the strategy.                // 这个模块包含 策略使用的常量
pub mod constants;

/// This module contains the core strategy implementation.              // 这个模块包含 核心策略的实现
pub mod strategy;

/// This module contains the core type definitions for the strategy.    // 这个模块包含 策略的核心类型定义
pub mod types;
