use wasm_bindgen::prelude::*;

use anyhow::Result;

use crate::app::App;

#[derive(Clone)]
#[wasm_bindgen]
pub struct WebHandle {
    runner: eframe::WebRunner,
}

#[wasm_bindgen]
impl WebHandle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        eframe::WebLogger::init(log::LevelFilter::Debug).ok();

        Self {
            runner: eframe::WebRunner::new(),
        }
    }

    #[wasm_bindgen]
    pub async fn start(
        &self,
        canvas_id: &str,
        host: &str,
        client_id: &str,
    ) -> Result<(), wasm_bindgen::JsValue> {
        let host = host.to_string();
        let client_id = client_id.to_string();

        let options = eframe::WebOptions::default();

        self.runner
            .start(
                canvas_id,
                options,
                Box::new(|cc| {
                    cc.egui_ctx.set_style(egui::Style {
                        visuals: egui::Visuals::dark(),
                        ..Default::default()
                    });

                    Box::new(App::new(cc, host, client_id))
                }),
            )
            .await
    }

    #[wasm_bindgen]
    pub fn destroy(&self) {
        self.runner.destroy()
    }

    #[wasm_bindgen]
    pub fn has_panicked(&self) -> bool {
        self.runner.has_panicked()
    }

    #[wasm_bindgen]
    pub fn panic_message(&self) -> Option<String> {
        self.runner.panic_summary().map(|s| s.message())
    }

    #[wasm_bindgen]
    pub fn panic_callstack(&self) -> Option<String> {
        self.runner.panic_summary().map(|s| s.callstack())
    }
}
