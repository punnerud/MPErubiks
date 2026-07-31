//! The Rubik's cube application (egui/eframe). Lib + thin bin split so
//! integration tests (screenshot harness) can drive the app.

pub mod app;
pub mod i18n;
pub mod lessons;
pub mod library;
pub mod persist;
pub mod platform;
pub mod screens;
pub mod widgets;
