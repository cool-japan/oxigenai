//! # oxigenai
//!
//! Pure Rust reimplementation of Digital Agency's lawsy-custom-bq (源内)
//! with Legalis-RS computational law verification.
//!
//! Provides a CLI and HTTP API for querying Japanese law databases,
//! verifying legal compliance, and generating structured legal reports.

// Allow dead code: full API surface, not all items called from every binary.
#![allow(dead_code)]

pub mod config;
pub mod error;
pub mod handlers;
pub mod models;
pub mod prompts;
pub mod services;
pub mod verifier;
