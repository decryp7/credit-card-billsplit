use std::sync::{Arc, Mutex};

use egui::{Align, Color32, CornerRadius, Key, KeyboardShortcut, Layout, Modifiers, RichText, Stroke};
use egui_extras::{Column, TableBuilder};

use crate::bill_reader::{last_four, BillReader, CreditCardBillReader, Transaction};
use crate::merchant::{self, Category, Merchant};
use crate::theme;

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

    /// When set, only transactions in this category are shown.
    #[serde(skip)]
    category: Option<Category>,
    /// Merchant worked out from each transaction's description, in the same order.
    #[serde(skip)]
    merchants: Vec<Merchant>,
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
            category: None,
            merchants: Vec::new(),
            inbox: Arc::new(Mutex::new(None)),
            loading: false,
            error: None,
        }
    }
}

impl BillSplitApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);

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
                self.merchants.clear();
                self.file_name = Some(loaded.file_name);
                self.filter.clear();
                self.category = None;
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

    /// Identifies the merchant of every transaction, unless already done for this statement.
    fn identify_merchants(&mut self) {
        if self.merchants.len() != self.transactions.len() {
            self.merchants = self.transactions.iter().map(|t| merchant::identify(&t.description)).collect();
        }
    }

    /// Indices of the transactions that pass the filter box and the category filter.
    fn visible(&self) -> Vec<usize> {
        (0..self.transactions.len())
            .filter(|&i| self.category.is_none_or(|c| self.merchants[i].category == c))
            .filter(|&i| matches(&self.filter, &self.transactions[i], &self.merchants[i]))
            .collect()
    }

    // ----------------------------------------------------------------------------
    // UI

    /// Site name and app title on the left, statement actions on the right.
    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add(egui::Hyperlink::from_label_and_url(theme::eyebrow("decryptology.net"), "https://decryptology.net")
                    .open_in_new_tab(true));
                ui.label(theme::display("Credit Card BillSplit", 28.0));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.transactions.is_empty() {
                    return;
                }
                let shortcut = ui.ctx().format_shortcut(&OPEN_SHORTCUT);
                if ui.add_enabled(!self.loading, theme::primary_button("Open statement…"))
                    .on_hover_text(format!("Open another statement ({shortcut})"))
                    .clicked()
                {
                    self.open_file_dialog(ui.ctx());
                }
                if ui.add(egui::Button::new("Close").corner_radius(CornerRadius::same(255))).clicked() {
                    self.transactions.clear();
                    self.file_name = None;
                    self.filter.clear();
                    self.category = None;
                }
            });
        });
    }

    fn footer(ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let text = |s: &str| RichText::new(s).size(12.0).color(theme::FAINT);
            let link = |s: &str| RichText::new(s).size(12.0).color(theme::GOLD_SOFT);
            ui.label(text("Developed by "));
            ui.add(egui::Hyperlink::from_label_and_url(link("decryp7"), "https://decryptology.net")
                .open_in_new_tab(true));
            ui.label(text("  ·  "));
            ui.add(egui::Hyperlink::from_label_and_url(link("source code"), "https://dev.decryptology.net/decryp7/credit-card-billsplit")
                .open_in_new_tab(true));
            ui.with_layout(Layout::right_to_left(Align::Center), egui::warn_if_debug_build);
        });
    }

    /// A softly lit tile with a caption and a large serif figure.
    fn summary_tile(ui: &mut egui::Ui, title: &str, value: String, color: Option<Color32>) {
        theme::tile_frame().show(ui, |ui| {
            ui.set_min_width(130.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if let Some(color) = color {
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(7.0, 7.0), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 3.5, color);
                    }
                    ui.label(theme::caption(title));
                });
                ui.label(theme::display(value, 30.0).color(color.unwrap_or(theme::GOLD_LIGHT)));
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
            ui.spacing_mut().item_spacing = egui::vec2(12.0, 12.0);
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
                Self::summary_tile(ui, "Unassigned", format_money(unassigned), Some(theme::WARN));
            }
        });
    }

    /// Spending per category as one proportional bar, with a chip per category that filters the
    /// table to it. Payments and the previous balance aren't spending, so they're left out.
    fn categories(&mut self, ui: &mut egui::Ui, cards: &[String]) {
        let mut totals: Vec<(Category, f64)> = Vec::new();
        for (transaction, merchant) in self.transactions.iter().zip(&self.merchants) {
            if merchant.category == Category::Payments {
                continue;
            }
            match totals.iter_mut().find(|(c, _)| *c == merchant.category) {
                Some((_, total)) => *total += transaction.amount,
                None => totals.push((merchant.category, transaction.amount)),
            }
        }
        totals.retain(|&(_, total)| total > 0.005);
        totals.sort_by(|a, b| b.1.total_cmp(&a.1));
        let spending: f64 = totals.iter().map(|(_, total)| total).sum();
        if totals.is_empty() {
            return;
        }

        theme::card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(theme::eyebrow("Spending by category"));
                ui.label(RichText::new(format_money(spending)).size(12.0).color(theme::FAINT));
            });

            // The bar: one segment per category, as wide as its share of the spending.
            let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 8.0), egui::Sense::hover());
            let painter = ui.painter();
            painter.rect_filled(rect, 4.0, theme::alpha(Color32::WHITE, 0x0a));
            let mut x = rect.left();
            for &(category, total) in &totals {
                let width = (total / spending) as f32 * rect.width();
                let segment = egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(width, rect.height()));
                let dimmed = self.category.is_some_and(|c| c != category);
                let color = category_color(category);
                painter.rect_filled(segment.shrink2(egui::vec2(1.0, 0.0)), 4.0,
                                    if dimmed { theme::alpha(color, 0x40) } else { color });
                x += width;
            }

            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                ui.spacing_mut().button_padding = egui::vec2(10.0, 3.0);
                for &(category, total) in &totals {
                    let color = category_color(category);
                    let selected = self.category == Some(category);
                    let mut job = egui::text::LayoutJob::default();
                    let font = |size| egui::FontId::proportional(size);
                    job.append("• ", 0.0, egui::TextFormat::simple(font(16.0), color));
                    job.append(category.label(), 0.0, egui::TextFormat::simple(font(13.0), theme::INK));
                    job.append(&format_money(total), 8.0, egui::TextFormat::simple(font(13.0), theme::GOLD_LIGHT));
                    job.append(&format!("{:.0}%", total / spending * 100.0), 6.0,
                               egui::TextFormat::simple(font(11.5), theme::FAINT));
                    let chip = egui::Button::new(job)
                        .fill(if selected { theme::alpha(color, 0x26) } else { Color32::TRANSPARENT })
                        .stroke(Stroke::new(1.0, if selected { color } else { theme::alpha(Color32::WHITE, 0x1a) }))
                        .corner_radius(CornerRadius::same(255));
                    let response = ui.add(chip).on_hover_ui(|ui| self.category_breakdown(ui, category, cards));
                    if response.clicked() {
                        self.category = if selected { None } else { Some(category) };
                    }
                }
            });
        });
    }

    /// Tooltip with how much of a category each card has been assigned.
    fn category_breakdown(&self, ui: &mut egui::Ui, category: Category, cards: &[String]) {
        let in_category = || self.transactions.iter().zip(&self.merchants).filter(|(_, m)| m.category == category);
        ui.label(theme::eyebrow(category.label()));
        egui::Grid::new("category_breakdown").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
            for (index, card) in cards.iter().enumerate() {
                let total: f64 = in_category()
                    .filter(|(t, _)| t.assigned_to.as_deref() == Some(card))
                    .map(|(t, _)| t.amount)
                    .sum();
                ui.label(RichText::new(format!("•••• {card}")).color(card_color(index)));
                ui.label(RichText::new(format_money(total)).color(theme::INK));
                ui.end_row();
            }
            let unassigned: f64 = in_category().filter(|(t, _)| t.assigned_to.is_none()).map(|(t, _)| t.amount).sum();
            if unassigned != 0.0 {
                ui.label(RichText::new("Unassigned").color(theme::WARN));
                ui.label(RichText::new(format_money(unassigned)).color(theme::INK));
                ui.end_row();
            }
        });
        ui.label(RichText::new("Click to show only this category").size(11.5).color(theme::FAINT));
    }

    /// Filter box and buttons to (re)assign the transactions shown in bulk.
    fn toolbar(&mut self, ui: &mut egui::Ui, cards: &[String]) {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.filter)
                .hint_text("🔍  Filter by merchant, category, country…")
                .margin(egui::Margin::symmetric(12, 6))
                .desired_width(300.0));
            if !self.filter.is_empty() && ui.small_button("✖").on_hover_text("Clear filter").clicked() {
                self.filter.clear();
            }
            if let Some(category) = self.category {
                let color = category_color(category);
                let chip = egui::Button::new(RichText::new(format!("{}  ✖", category.label())).color(color))
                    .fill(theme::alpha(color, 0x1a))
                    .stroke(Stroke::new(1.0, theme::alpha(color, 0x80)))
                    .corner_radius(CornerRadius::same(255));
                if ui.add(chip).on_hover_text("Show all categories").clicked() {
                    self.category = None;
                }
            }

            ui.add_space(12.0);
            ui.label(theme::caption("Assign shown to"));
            for (index, card) in cards.iter().enumerate() {
                let color = card_color(index);
                let button = egui::Button::new(RichText::new(format!("•••• {card}")).strong().color(color))
                    .fill(theme::alpha(color, 0x14))
                    .stroke(Stroke::new(1.0, theme::alpha(color, 0x80)))
                    .corner_radius(CornerRadius::same(255));
                if ui.add(button).on_hover_text("Assign all transactions shown below to this card").clicked() {
                    for i in self.visible() {
                        self.transactions[i].assigned_to = Some(card.clone());
                    }
                }
            }
            if ui.add(egui::Button::new("Reset").corner_radius(CornerRadius::same(255)))
                .on_hover_text("Assign every transaction back to the card it was made on")
                .clicked()
            {
                for transaction in self.transactions.iter_mut() {
                    transaction.assigned_to = Some(last_four(&transaction.card).to_string());
                }
            }

            if let Some(file_name) = &self.file_name {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add(egui::Label::new(RichText::new(file_name).family(theme::serif_italic()).size(17.0).color(theme::MUTED))
                        .truncate());
                });
            }
        });
    }

    /// One row per transaction, with a button per card to choose who pays for it.
    /// Clicking the selected card again leaves the transaction unassigned.
    fn table(&mut self, ui: &mut egui::Ui, cards: &[String]) {
        // Indices into `self.transactions`, so edits made in the table apply to the real rows.
        let visible = self.visible();
        let row_height = 46.0;
        let heading = |ui: &mut egui::Ui, text: &str| { ui.label(theme::eyebrow(text).size(10.0)); };

        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::auto().at_least(70.0))
            .column(Column::auto().at_least(240.0).clip(true))
            .column(Column::auto().at_least(270.0).clip(true))
            .column(Column::auto().at_least(130.0))
            .column(Column::auto().at_least(110.0))
            .column(Column::remainder())
            .auto_shrink(false)
            .header(30.0, |mut header| {
                header.col(|ui| heading(ui, "Date"));
                header.col(|ui| heading(ui, "Description"));
                header.col(|ui| heading(ui, "Merchant"));
                header.col(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| heading(ui, "Amount (SGD)"));
                });
                header.col(|ui| { ui.add_space(16.0); heading(ui, "Card"); });
                header.col(|ui| heading(ui, "Assigned to"));
            })
            .body(|body| {
                // Only the rows on screen are laid out, which keeps long statements fast.
                body.rows(row_height, visible.len(), |mut row| {
                    let index = visible[row.index()];
                    let transaction = &mut self.transactions[index];
                    let merchant = &self.merchants[index];
                    row.col(|ui| { ui.label(RichText::new(&transaction.date).color(theme::MUTED)); });
                    row.col(|ui| {
                        ui.label(RichText::new(&transaction.description).color(theme::BODY))
                            .on_hover_text(&transaction.description);
                    });
                    row.col(|ui| Self::merchant_cell(ui, merchant));
                    row.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let mut text = RichText::new(format!("{:.2}", transaction.amount)).monospace();
                            text = if transaction.amount < 0.0 {
                                text.color(theme::CREDIT)
                            } else if transaction.amount >= LARGE_AMOUNT {
                                text.color(theme::GOLD_LIGHT)
                            } else {
                                text.color(theme::BODY)
                            };
                            ui.label(text);
                        });
                    });
                    row.col(|ui| {
                        ui.add_space(16.0);
                        ui.label(RichText::new(format!("•••• {}", last_four(&transaction.card))).color(theme::FAINT))
                            .on_hover_text(&transaction.card);
                    });
                    row.col(|ui| {
                        ui.spacing_mut().button_padding = egui::vec2(10.0, 2.0);
                        ui.spacing_mut().item_spacing.x = 6.0;
                        for (index, card) in cards.iter().enumerate() {
                            let selected = transaction.assigned_to.as_deref() == Some(card);
                            let color = card_color(index);
                            let chip = if selected {
                                egui::Button::new(RichText::new(card).strong().color(color))
                                    .fill(theme::alpha(color, 0x26))
                                    .stroke(Stroke::new(1.0, theme::alpha(color, 0xb0)))
                            } else {
                                egui::Button::new(RichText::new(card).color(theme::FAINT))
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::new(1.0, theme::alpha(Color32::WHITE, 0x14)))
                            };
                            if ui.add(chip.corner_radius(CornerRadius::same(255))).clicked() {
                                transaction.assigned_to = if selected { None } else { Some(card.clone()) };
                            }
                        }
                    });
                });
            });
    }

    /// Merchant name on top; category, place and payment processor underneath.
    fn merchant_cell(ui: &mut egui::Ui, merchant: &Merchant) {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
            ui.add(egui::Label::new(RichText::new(&merchant.name).family(theme::semibold()).size(13.5).color(theme::INK_BRIGHT))
                .truncate());
            ui.horizontal(|ui| {
                ui.label(RichText::new(merchant.category.label()).size(11.5).color(category_color(merchant.category)));
                let mut details = String::new();
                if let Some(place) = &merchant.place {
                    details.push_str(&format!("  ·  {place}"));
                }
                if let Some(via) = merchant.via {
                    details.push_str(&format!("  ·  via {via}"));
                }
                ui.add(egui::Label::new(RichText::new(details).size(11.5).color(theme::FAINT)).truncate());
            });
        });
    }

    /// Shown when no statement is open.
    fn empty_state(&mut self, ui: &mut egui::Ui) {
        theme::paint_glow(ui.painter(), ui.max_rect());
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() / 2.0 - 260.0).max(24.0));
            ui.label(theme::eyebrow("Split the bill"));
            ui.add_space(10.0);
            ui.label(theme::display("Who spent what?", 56.0));
            ui.label(RichText::new("Your credit card statement, card by card")
                .family(theme::serif_italic())
                .size(22.0)
                .color(Color32::from_rgb(0xe9, 0xd7, 0xb0)));
            ui.add_space(14.0);
            theme::flourish(ui);
            ui.add_space(14.0);
            let body = |s: &str| RichText::new(s).size(15.5).color(theme::BODY);
            ui.label(body("Open a Citibank credit card eStatement (PDF) to list its transactions."));
            ui.label(body("Each transaction is assigned to the card it was made on — click a card to reassign it."));
            ui.add_space(22.0);
            let open = theme::primary_button("Open statement…").min_size(egui::vec2(200.0, 42.0));
            if ui.add(open).on_hover_text(ui.ctx().format_shortcut(&OPEN_SHORTCUT)).clicked() {
                self.open_file_dialog(ui.ctx());
            }
            ui.add_space(6.0);
            ui.label(RichText::new("or drag and drop a PDF here").size(13.0).color(theme::FAINT));
            ui.add_space(40.0);

            let highlights = [
                ("Private", "The PDF is read on your device. Nothing is uploaded."),
                ("Per card", "Every charge starts on the card it was made on."),
                ("Flexible", "Filter, then assign whole groups of charges at once."),
            ];
            let width = ui.available_width().min(3.0 * 210.0 + 2.0 * 12.0);
            ui.allocate_ui(egui::vec2(width, 0.0), |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(12.0, 12.0);
                    for (title, text) in highlights {
                        theme::tile_frame().show(ui, |ui| {
                            ui.set_width(176.0);
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 4.0;
                                ui.label(theme::display(title, 24.0).color(theme::GOLD_LIGHT));
                                ui.label(RichText::new(text).size(13.0).color(theme::BODY));
                            });
                        });
                    }
                });
            });
        });
    }

    /// A slim banner explaining why the last statement couldn't be opened.
    fn error_banner(ui: &mut egui::Ui, error: &str) {
        egui::Frame::new()
            .fill(theme::alpha(theme::ERROR, 0x1a))
            .stroke(Stroke::new(1.0, theme::alpha(theme::ERROR, 0x66)))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(egui::Margin::symmetric(14, 8))
            .show(ui, |ui| {
                ui.colored_label(theme::ERROR, format!("⚠  {error}"));
            });
    }

    /// Dims the window while a file is dragged over it.
    fn drop_overlay(ctx: &egui::Context) {
        if ctx.input(|i| i.raw.hovered_files.is_empty()) {
            return;
        }
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop_overlay")));
        let rect = ctx.content_rect();
        painter.rect_filled(rect, 0.0, theme::alpha(theme::BG, 0xe0));
        painter.rect_stroke(rect.shrink(24.0), 18.0, Stroke::new(1.5, theme::alpha(theme::GOLD, 0xb0)),
                            egui::StrokeKind::Inside);
        painter.text(rect.center() - egui::vec2(0.0, 14.0), egui::Align2::CENTER_CENTER, "Drop the statement",
                     egui::FontId::new(40.0, theme::serif()), theme::INK_BRIGHT);
        painter.text(rect.center() + egui::vec2(0.0, 24.0), egui::Align2::CENTER_CENTER, "PDF ESTATEMENTS ONLY",
                     egui::FontId::proportional(11.0), theme::GOLD);
    }
}

/// Whether a transaction matches the filter text (case-insensitive).
fn matches(filter: &str, transaction: &Transaction, merchant: &Merchant) -> bool {
    let filter = filter.trim().to_lowercase();
    filter.is_empty()
        || transaction.description.to_lowercase().contains(&filter)
        || merchant.search_text().contains(&filter)
        || transaction.date.to_lowercase().contains(&filter)
        || transaction.card.contains(&filter)
}

/// Formats an amount as "$1234.56" or "-$1234.56".
fn format_money(amount: f64) -> String {
    let sign = if amount < 0.0 { "-" } else { "" };
    format!("{sign}${:.2}", amount.abs())
}

/// A gentle tint per category, so similar spending stands out down the table.
fn category_color(category: Category) -> Color32 {
    match category {
        Category::Dining => Color32::from_rgb(0xe8, 0xa8, 0x7c),
        Category::Groceries => Color32::from_rgb(0xa8, 0xcf, 0x8e),
        Category::Transport => Color32::from_rgb(0x8f, 0xb4, 0xe8),
        Category::Shopping => Color32::from_rgb(0xe2, 0xb3, 0x5c),
        Category::Travel => Color32::from_rgb(0x84, 0xcc, 0xc8),
        Category::Subscriptions => Color32::from_rgb(0xbe, 0xa6, 0xe8),
        Category::Bills => Color32::from_rgb(0xc9, 0xc4, 0x7a),
        Category::Health => Color32::from_rgb(0xe8, 0x93, 0xa8),
        Category::Leisure => Color32::from_rgb(0x9c, 0xd6, 0xa8),
        Category::Fees => theme::ERROR,
        Category::Payments => theme::CREDIT,
        Category::Other => theme::MUTED,
    }
}

/// A distinct color per card, used to tell cards apart at a glance. Warm and muted to sit with the gold.
fn card_color(index: usize) -> Color32 {
    const COLORS: [Color32; 6] = [
        theme::GOLD,
        Color32::from_rgb(0x8f, 0xb4, 0xe8), // moonlight blue
        Color32::from_rgb(0xe8, 0x93, 0x8a), // lantern rose
        Color32::from_rgb(0x84, 0xcc, 0xb0), // jade
        Color32::from_rgb(0xbe, 0xa6, 0xe8), // lavender
        Color32::from_rgb(0xc9, 0xc4, 0x7a), // osmanthus
    ];
    COLORS[index % COLORS.len()]
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

        let top = egui::Panel::top("top_panel")
            .frame(egui::Frame::new().fill(theme::BG).inner_margin(egui::Margin::symmetric(24, 12)))
            .show(ui, |ui| self.header(ui));
        let rule = top.response.rect;
        theme::paint_rule(ui.painter(), rule.left(), rule.right(), rule.bottom(), theme::alpha(theme::GOLD, 0x8c));
        egui::Panel::bottom("bottom_panel")
            .frame(egui::Frame::new().fill(theme::BG).inner_margin(egui::Margin::symmetric(24, 8)))
            .show(ui, Self::footer);

        let cards = self.cards();
        self.identify_merchants();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::NAVY).inner_margin(egui::Margin::symmetric(24, 18)))
            .show(ui, |ui| {
                if self.loading {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new("Reading statement…").family(theme::serif_italic()).size(18.0)
                            .color(theme::GOLD_SOFT));
                    });
                }
                if let Some(error) = &self.error {
                    Self::error_banner(ui, error);
                }

                if self.transactions.is_empty() {
                    if !self.loading {
                        self.empty_state(ui);
                    }
                    return;
                }

                self.summary(ui, &cards);
                ui.add_space(10.0);
                self.categories(ui, &cards);
                ui.add_space(10.0);
                self.toolbar(ui, &cards);
                ui.add_space(8.0);
                theme::card_frame().show(ui, |ui| self.table(ui, &cards));
            });

        Self::drop_overlay(ui.ctx());
    }
}
