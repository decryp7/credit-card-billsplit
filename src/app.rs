use rfd::{AsyncFileDialog, AsyncMessageDialog, FileHandle, MessageButtons, MessageLevel};
use std::default::Default;
use std::sync::{Arc, Mutex};
use egui::{Button, Color32, Layout, Margin, RichText, Vec2, Window};
use itertools::Itertools;
use crate::bill_reader::{BillReader, CreditCardBillReader, Transaction, JOINT_TAG, PERSONAL_TAG};

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

    fn build_button(ui: &mut egui::Ui, transaction: &mut Transaction, content: &str){
        let mut button_text = RichText::new(content);
        if transaction.tags.contains(&content.to_string()) {
            button_text = button_text.strong().underline();
        }
        let button = Button::new(button_text);

        if ui.add(button).clicked() {
            match transaction.tags.iter().position(|t| {t == &content.to_string()}) {
                None => {
                    transaction.tags.clear();
                    transaction.tags.push(content.to_string());
                }
                Some(s) => {
                    transaction.tags.remove(s);
                }
            }
        };
    }

    fn build_table(&mut self, ui: &mut egui::Ui) {
        use egui_extras::{Column, TableBuilder};

        let available_height = ui.available_height();
        let mut table = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::remainder())
            .max_scroll_height(0.0)
            .max_scroll_height(available_height)
            .sense(egui::Sense::click());

        table
            .header(20.0, |mut header| {
                header.col(|ui|{
                   ui.strong("Date");
                });
                header.col(|ui|{
                    ui.strong("Description");
                });
                header.col(|ui|{
                    ui.strong("Amount (SGD)");
                });
                header.col(|ui|{
                    ui.strong("Card");
                });
                header.col(|ui|{
                    ui.strong("Tags");
                });
            })
            .body(|mut body|{
                let mut t = self.transactions.lock().unwrap();
                let mut t = &mut *t;
                for mut transaction in t {
                    body.row(18.0, |mut row |{
                       row.col(|ui|{
                          ui.label(&transaction.date);
                       });
                        row.col(|ui|{
                            ui.label(&transaction.description);
                        });
                        row.col(|ui|{
                            ui.label(&transaction.amount.to_string());
                        });
                        row.col(|ui|{
                            ui.label(&transaction.card);
                        });
                        row.col(|ui|{
                            //ui.label(&transaction.tags.join(", "));
                            Self::build_button(ui, transaction, PERSONAL_TAG);
                            Self::build_button(ui, transaction, JOINT_TAG);
                        });
                    });
                }
            });
    }
}

impl eframe::App for BillSplitApp {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(1)
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
                            true
                        };

                        async_std::task::block_on(future);
                        ui.close_menu();
                    }
                });
            });
        });

        egui::TopBottomPanel::bottom("bottom-panel")
            .show(ctx, |ui|{
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("Developed by ");
                    ui.add(egui::Hyperlink::from_label_and_url("decryp7", "https://decryptology.net")
                        .open_in_new_tab(true));
                    ui.label(".");
                    ui.separator();
                    ui.add(egui::Hyperlink::from_label_and_url("(source code)", "https://dev.decryptology.net/decryp7/credit-card-billsplit")
                        .open_in_new_tab(true));
                    ui.separator();
                    egui::warn_if_debug_build(ui);
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui|{
                    ui.horizontal(|ui|{
                        let t = self.transactions.lock().unwrap();
                        let mut total = 0f64;
                        for transaction in &*t {
                            total += transaction.amount;
                        }
                        ui.label("Total: ");
                        ui.label(RichText::new(format!("${:.2}", total))
                            .strong()
                            .size(20.0));
                        ui.separator();

                        let mut personal_total = 0f64;
                        for transaction in &*t.iter()
                            .filter(|t| t.tags.contains(&PERSONAL_TAG.to_string())).collect::<Vec<&Transaction>>() {
                            personal_total += transaction.amount;
                        }
                        ui.label("Personal: ");
                        ui.label(RichText::new(format!("${:.2}", personal_total))
                            .strong()
                            .size(20.0));
                        ui.separator();

                        let mut joint_total = 0f64;
                        for transaction in &*t.iter()
                            .filter(|t| t.tags.contains(&JOINT_TAG.to_string())).collect::<Vec<&Transaction>>() {
                            personal_total += transaction.amount;
                        }
                        ui.label("Joint: ");
                        ui.label(RichText::new(format!("${:.2}", personal_total))
                            .strong()
                            .size(20.0));
                        ui.separator();

                    });
                    self.build_table(ui);
            });
        });
    }
}
