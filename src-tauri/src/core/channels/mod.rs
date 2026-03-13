//! Channel management module
//!
//! This module provides channel configuration management and platform-specific
//! integrations (e.g., WhatsApp, Feishu, Discord, etc.).

pub mod config;
pub mod whatsapp;

pub use config::{Channel, ChannelStatus, ChannelManager, ChannelConfig};
pub use whatsapp::{WhatsAppManager, WhatsAppLoginState, WhatsAppLoginEvent, QRCodeData};
