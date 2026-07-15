#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(deprecated)]

mod app;
mod command;
mod document;
mod git;
mod icon;
mod security;
mod settings;
mod syntax;
mod workspace;

use app::OrionApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Orion IDE")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([760.0, 480.0])
            .with_icon(icon::app_icon()),
        ..Default::default()
    };

    eframe::run_native("Orion IDE", native_options, Box::new(|cc| Ok(Box::new(OrionApp::new(cc)))))
}
