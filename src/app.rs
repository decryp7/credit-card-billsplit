use eframe::{Error, WebLogger};
use log::{log, Level, LevelFilter};
use pdfium_render::prelude::{PdfDocument, PdfDocumentMetadataTagType, Pdfium, PdfiumError};
use rfd::{AsyncFileDialog, AsyncMessageDialog, FileHandle, MessageButtons, MessageLevel};
use wasm_bindgen_futures::spawn_local;
use std::default::Default;
use std::sync::{Arc, Mutex};
use egui::{Layout, Margin, Vec2, Window};
use egui::UiKind::ScrollArea;
use itertools::Itertools;
use crate::bill_reader::{BillReader, CreditCardBillReader, Transaction};

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct BillSplitApp {
    transactions: Arc<Mutex<Vec<Transaction>>>
}

impl Default for BillSplitApp {
    fn default() -> Self {
        Self {
            transactions: Arc::new(Mutex::new(Vec::<Transaction>::new()))
        }
    }
}

impl BillSplitApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return eframe::get_value::<BillSplitApp>(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }
}

impl eframe::App for BillSplitApp {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                egui::widgets::global_dark_light_mode_switch(ui);
                ui.separator();
                ui.menu_button("File", |ui| {
                    if ui.button("Open bill...").clicked() {
                        //https://users.rust-lang.org/t/how-can-i-read-a-file-from-disk-by-filedialog-on-wasm/97868/2
                        let transactions = Arc::clone(&self.transactions);
                        let future = async move {
                            let file = AsyncFileDialog::new()
                                .add_filter("pdf", &["pdf"])
                                .set_directory("/")
                                .pick_file()
                                .await;
                            match file {
                                None => {}
                                Some(f) => {
                                    let bill_reader = CreditCardBillReader::default();
                                    let data = f.read().await;
                                    let transactions_results = bill_reader.read(data);
                                    // log!(Level::Info, "Transactions count: {}", transactions.len());
                                    // let mut total  = 0.0f64;
                                    // for transaction in transactions {
                                    //     log!(Level::Info, "{}", transaction);
                                    //     total += transaction.amount;
                                    // }
                                    // log!(Level::Info, "Total: ${:.2}", total);
                                    // for transaction in transactions {
                                    //     self.transactions.push(transaction);
                                    // }
                                    let mut t = transactions.lock().unwrap();
                                    t.clear();
                                    for transaction in transactions_results {
                                        t.push(transaction);
                                    }
                                }
                            }
                        };

                        async_std::task::block_on(future);
                    }
                });
            });
        });

        egui::TopBottomPanel::bottom("bottom-panel")
            .show(ctx, |ui|{
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("Developed by ");
                    ui.hyperlink_to("decryp7", "https://decryptology.net");
                    ui.label(".");
                    ui.separator();
                    ui.hyperlink_to("(source code)", "https://dev.decryptology.net/decryp7/credit-card-billsplit");
                    ui.separator();
                    egui::warn_if_debug_build(ui);
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui|{
                let t = self.transactions.lock().unwrap();
                // for transaction in &*t {
                //     log!(Level::Info, "{}", transaction);
                // }
                ui.label(&*t.iter().clone().join("\n"));
            });
        });
    }
}
