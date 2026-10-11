use std::sync::{Arc, Mutex};

use egui::{Align, Color32, Key, KeyboardShortcut, Layout, Modifiers, RichText};
use egui_extras::{Column, TableBuilder};

use crate::bill_reader::{last_four, BillReader, CreditCardBillReader, Transaction};

const OPEN_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
/// Amounts from this value onwards are highlighted in the table.
const LARGE_AMOUNT: f64 = 45.0;

/// Result of reading a statement on a background thread (native) or task (web).
struct LoadedStatement {
    file_name: String,
    transactions: Result<Vec<Transaction>, String>,
}

/// Hands a loaded statement from the background loader back to the UI thread.
type Inbox = Arc<Mutex<Option<LoadedStatement>>>;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct BillSplitApp {
    /// Transactions of the open statement, including the card each one is assigned to.
    transactions: Vec<Transaction>,
    /// Name of the open statement file.
    file_name: Option<String>,
    /// Text typed in the filter box; only matching transactions are shown.
    filter: String,

    #[serde(skip)]
    inbox: Inbox,
    /// True while a statement is being read.
    #[serde(skip)]
    loading: bool,
    /// Error from the last attempt to open a statement.
    #[serde(skip)]
    error: Option<String>,
}

impl Default for BillSplitApp {
    fn default() -> Self {
        Self {
            transactions: Vec::new(),
            file_name: None,
            filter: String::new(),
            inbox: Arc::new(Mutex::new(None)),
            loading: false,
            error: None,
        }
    }
}

impl BillSplitApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return eframe::get_value::<BillSplitApp>(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }

    /// Last 4 digits of every card in the statement, in order of appearance.
    fn cards(&self) -> Vec<String> {
        let mut cards: Vec<String> = Vec::new();
        for transaction in &self.transactions {
            let card = last_four(&transaction.card);
            if !cards.iter().any(|c| c == card) {
                cards.push(card.to_string());
            }
        }
        cards
    }

    // ----------------------------------------------------------------------------
    // Loading

    /// Reads the transactions out of the PDF bytes. Runs off the UI thread.
    fn parse(file_name: String, data: Vec<u8>) -> LoadedStatement {
        let transactions = CreditCardBillReader::new().and_then(|reader| reader.read(data));
        LoadedStatement { file_name, transactions }
    }

    /// Passes the result to the UI and wakes it up so it is picked up on the next frame.
    fn deliver(inbox: &Inbox, ctx: &egui::Context, loaded: LoadedStatement) {
        *inbox.lock().unwrap() = Some(loaded);
        ctx.request_repaint();
    }

    /// Lets the user pick a statement and reads it in the background.
    fn open_file_dialog(&mut self, ctx: &egui::Context) {
        let inbox = Arc::clone(&self.inbox);
        let ctx = ctx.clone();

        // Native: the dialog is modal, so show it right away and only read the file on another thread.
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(path) = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).pick_file() else {
                return;
            };
            self.start_loading();
            std::thread::spawn(move || {
                let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let loaded = match std::fs::read(&path) {
                    Ok(data) => Self::parse(file_name, data),
                    Err(e) => LoadedStatement { file_name, transactions: Err(e.to_string()) },
                };
                Self::deliver(&inbox, &ctx, loaded);
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            // The browser file picker can only be awaited, and cancelling it never resolves on some
            // browsers, so no spinner is shown while it is open.
            wasm_bindgen_futures::spawn_local(async move {
                let Some(file) = rfd::AsyncFileDialog::new().add_filter("PDF", &["pdf"]).pick_file().await else {
                    return;
                };
                let data = file.read().await;
                Self::deliver(&inbox, &ctx, Self::parse(file.file_name(), data));
            });
        }
    }

    /// Reads a PDF that was dragged onto the window.
    fn open_dropped_file(&mut self, ctx: &egui::Context, file: egui::DroppedFileHandle) {
        let inbox = Arc::clone(&self.inbox);
        let ctx = ctx.clone();
        let file_name = file.path().file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.start_loading();

        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let loaded = match file.bytes() {
                Ok(data) => Self::parse(file_name, data),
                Err(e) => LoadedStatement { file_name, transactions: Err(e) },
            };
            Self::deliver(&inbox, &ctx, loaded);
        });

        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            let loaded = match file.bytes_async().await {
                Ok(data) => Self::parse(file_name, data),
                Err(e) => LoadedStatement { file_name, transactions: Err(e) },
            };
            Self::deliver(&inbox, &ctx, loaded);
        });
    }

    fn start_loading(&mut self) {
        self.loading = true;
        self.error = None;
    }

    /// Picks up a statement loaded in the background, replacing the current one on success.
    fn receive_loaded_statement(&mut self) {
        let Some(loaded) = self.inbox.lock().unwrap().take() else {
            return;
        };
        self.loading = false;
        match loaded.transactions {
            Ok(transactions) if transactions.is_empty() => {
                self.error = Some(format!("No transactions found in {}.", loaded.file_name));
            }
            Ok(transactions) => {
                self.transactions = transactions;
                self.file_name = Some(loaded.file_name);
                self.filter.clear();
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Opens the first PDF dropped onto the window, if any.
    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(file) = dropped.into_iter().find(|f| {
            f.path().extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
        }) {
            self.open_dropped_file(ctx, file);
        }
    }

    // ----------------------------------------------------------------------------
    // UI

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                let open = egui::Button::new("Open statement…")
                    .shortcut_text(ui.ctx().format_shortcut(&OPEN_SHORTCUT));
                if ui.add(open).clicked() {
                    self.open_file_dialog(ui.ctx());
                }
                if ui.add_enabled(!self.transactions.is_empty(), egui::Button::new("Close statement")).clicked() {
                    self.transactions.clear();
                    self.file_name = None;
                    self.filter.clear();
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                egui::widgets::global_theme_preference_switch(ui);
            });
        });
    }

    fn footer(ui: &mut egui::Ui) {
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
    }

    /// A small framed box with a caption and a large value.
    fn summary_tile(ui: &mut egui::Ui, title: &str, value: String, color: Option<Color32>) {
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::symmetric(12, 6))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).small().weak());
                    let mut text = RichText::new(value).strong().size(20.0);
                    if let Some(color) = color {
                        text = text.color(color);
                    }
                    ui.label(text);
                });
            });
    }

    /// Statement total plus the amount assigned to each card, so the bill can be split.
    fn summary(&self, ui: &mut egui::Ui, cards: &[String]) {
        let total: f64 = self.transactions.iter().map(|t| t.amount).sum();
        let unassigned: f64 = self.transactions.iter()
            .filter(|t| t.assigned_to.is_none())
            .map(|t| t.amount)
            .sum();

        ui.horizontal_wrapped(|ui| {
            Self::summary_tile(ui, "Transactions", self.transactions.len().to_string(), None);
            Self::summary_tile(ui, "Total", format_money(total), None);
            for (index, card) in cards.iter().enumerate() {
                let card_total: f64 = self.transactions.iter()
                    .filter(|t| t.assigned_to.as_deref() == Some(card))
                    .map(|t| t.amount)
                    .sum();
                Self::summary_tile(ui, &format!("Card •••• {card}"), format_money(card_total), Some(card_color(index)));
            }
            if self.transactions.iter().any(|t| t.assigned_to.is_none()) {
                Self::summary_tile(ui, "Unassigned", format_money(unassigned), Some(ui.visuals().warn_fg_color));
            }
        });
    }

    /// Filter box and buttons to (re)assign the transactions shown in bulk.
    fn toolbar(&mut self, ui: &mut egui::Ui, cards: &[String]) {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.filter)
                .hint_text("🔍 Filter by date, description or card")
                .desired_width(280.0));
            if !self.filter.is_empty() && ui.small_button("✖").on_hover_text("Clear filter").clicked() {
                self.filter.clear();
            }

            ui.separator();
            ui.label("Assign shown to:");
            for (index, card) in cards.iter().enumerate() {
                let button = egui::Button::new(RichText::new(format!("•••• {card}")).color(card_color(index)));
                if ui.add(button).on_hover_text("Assign all transactions shown below to this card").clicked() {
                    let card = Some(card.clone());
                    let filter = self.filter.clone();
                    for transaction in self.transactions.iter_mut() {
                        if matches(&filter, transaction) {
                            transaction.assigned_to = card.clone();
                        }
                    }
                }
            }
            if ui.button("Reset").on_hover_text("Assign every transaction back to the card it was made on").clicked() {
                for transaction in self.transactions.iter_mut() {
                    transaction.assigned_to = Some(last_four(&transaction.card).to_string());
                }
            }

            if let Some(file_name) = &self.file_name {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(file_name).weak());
                });
            }
        });
    }

    /// One row per transaction, with a button per card to choose who pays for it.
    /// Clicking the selected card again leaves the transaction unassigned.
    fn table(&mut self, ui: &mut egui::Ui, cards: &[String]) {
        // Indices into `self.transactions`, so edits made in the table apply to the real rows.
        let visible: Vec<usize> = (0..self.transactions.len())
            .filter(|&i| matches(&self.filter, &self.transactions[i]))
            .collect();
        let row_height = ui.spacing().interact_size.y + 4.0;

        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::auto().at_least(60.0))
            .column(Column::auto().at_least(240.0).clip(true))
            .column(Column::auto().at_least(90.0))
            .column(Column::auto().at_least(80.0))
            .column(Column::remainder())
            .auto_shrink(false)
            .header(24.0, |mut header| {
                header.col(|ui| { ui.strong("Date"); });
                header.col(|ui| { ui.strong("Description"); });
                header.col(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| { ui.strong("Amount (SGD)"); });
                });
                header.col(|ui| { ui.strong("Card"); });
                header.col(|ui| { ui.strong("Assigned to"); });
            })
            .body(|body| {
                // Only the rows on screen are laid out, which keeps long statements fast.
                body.rows(row_height, visible.len(), |mut row| {
                    let transaction = &mut self.transactions[visible[row.index()]];
                    row.col(|ui| { ui.label(&transaction.date); });
                    row.col(|ui| { ui.label(&transaction.description).on_hover_text(&transaction.description); });
                    row.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let mut text = RichText::new(format!("{:.2}", transaction.amount)).monospace();
                            if transaction.amount < 0.0 {
                                text = text.color(credit_color(ui));
                            } else if transaction.amount >= LARGE_AMOUNT {
                                text = text.strong();
                            }
                            ui.label(text);
                        });
                    });
                    row.col(|ui| {
                        ui.label(format!("•••• {}", last_four(&transaction.card)))
                            .on_hover_text(&transaction.card);
                    });
                    row.col(|ui| {
                        for (index, card) in cards.iter().enumerate() {
                            let selected = transaction.assigned_to.as_deref() == Some(card);
                            let mut text = RichText::new(card);
                            if selected {
                                text = text.strong().color(card_color(index));
                            }
                            if ui.add(egui::Button::selectable(selected, text)).clicked() {
                                transaction.assigned_to = if selected { None } else { Some(card.clone()) };
                            }
                        }
                    });
                });
            });
    }

    /// Shown when no statement is open.
    fn empty_state(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 3.0);
            ui.heading("Split your credit card bill");
            ui.add_space(8.0);
            ui.label("Open a Citibank credit card eStatement (PDF) to list its transactions.");
            ui.label("Each transaction is assigned to the card it was made on — click a card to reassign it.");
            ui.add_space(12.0);
            if ui.add(egui::Button::new(RichText::new("📂 Open statement…").size(16.0))).clicked() {
                self.open_file_dialog(ui.ctx());
            }
            ui.add_space(4.0);
            ui.label(RichText::new("or drag and drop a PDF here").weak());
        });
    }

    /// Dims the window while a file is dragged over it.
    fn drop_overlay(ctx: &egui::Context) {
        if ctx.input(|i| i.raw.hovered_files.is_empty()) {
            return;
        }
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop_overlay")));
        let rect = ctx.content_rect();
        painter.rect_filled(rect, 0.0, Color32::from_black_alpha(180));
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, "Drop the PDF statement to open it",
                     egui::FontId::proportional(24.0), Color32::WHITE);
    }
}

/// Whether a transaction matches the filter text (case-insensitive).
fn matches(filter: &str, transaction: &Transaction) -> bool {
    let filter = filter.trim().to_lowercase();
    filter.is_empty()
        || transaction.description.to_lowercase().contains(&filter)
        || transaction.date.to_lowercase().contains(&filter)
        || transaction.card.contains(&filter)
}

/// Formats an amount as "$1234.56" or "-$1234.56".
fn format_money(amount: f64) -> String {
    let sign = if amount < 0.0 { "-" } else { "" };
    format!("{sign}${:.2}", amount.abs())
}

/// A distinct color per card, used to tell cards apart at a glance.
fn card_color(index: usize) -> Color32 {
    const COLORS: [Color32; 6] = [
        Color32::from_rgb(66, 150, 250),
        Color32::from_rgb(230, 140, 40),
        Color32::from_rgb(170, 110, 230),
        Color32::from_rgb(40, 180, 170),
        Color32::from_rgb(220, 90, 120),
        Color32::from_rgb(150, 160, 60),
    ];
    COLORS[index % COLORS.len()]
}

/// Color for credits (refunds and payments), readable in both themes.
fn credit_color(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode { Color32::from_rgb(110, 200, 120) } else { Color32::from_rgb(30, 130, 50) }
}

impl eframe::App for BillSplitApp {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(1)
    }

    /// Called before every frame; handles work that doesn't draw anything.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.receive_loaded_statement();
        if !self.loading {
            self.handle_dropped_files(ctx);
        }
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if ui.input_mut(|i| i.consume_shortcut(&OPEN_SHORTCUT)) && !self.loading {
            self.open_file_dialog(ui.ctx());
        }

        egui::Panel::top("top_panel").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::bottom("bottom_panel").show(ui, Self::footer);

        let cards = self.cards();
        egui::CentralPanel::default().show(ui, |ui| {
            if self.loading {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Reading statement…");
                });
            }
            if let Some(error) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, format!("⚠ {error}"));
            }

            if self.transactions.is_empty() {
                if !self.loading {
                    self.empty_state(ui);
                }
                return;
            }

            self.summary(ui, &cards);
            ui.add_space(4.0);
            self.toolbar(ui, &cards);
            ui.separator();
            self.table(ui, &cards);
        });

        Self::drop_overlay(ui.ctx());
    }
}
