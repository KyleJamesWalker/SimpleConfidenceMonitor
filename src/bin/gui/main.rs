#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod logs;

use eframe::egui;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::writer::MakeWriterExt;

use crate::app::App;
use crate::logs::LogBuffer;

fn main() -> eframe::Result {
    let logs = LogBuffer::default();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with_ansi(false)
        .with_target(false)
        .with_writer(logs.clone().and(std::io::stderr))
        .init();

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Simple Confidence Monitor")
            .with_inner_size([760.0, 680.0])
            .with_min_inner_size([520.0, 440.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!(
                    "../../../packaging/icon/icon-256.png"
                ))
                .expect("embedded icon"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "Simple Confidence Monitor",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, runtime, logs)))),
    )
}
