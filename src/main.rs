#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use pdfium_render::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn log_page_metrics_to_console(url: String) {
    let pdfium = Pdfium::default();

    let document = pdfium.load_pdf_from_fetch(url, None).await.unwrap();

    // Output metadata and form information for the PDF file to the console.

    log::info!("PDF file version: {:#?}", document.version());

    log::info!("PDF metadata tags:");

    document
        .metadata()
        .iter()
        .enumerate()
        .for_each(|(index, tag)| log::info!("{}: {:#?} = {}", index, tag.tag_type(), tag.value()));

    let pages = document.pages();

    match document.form() {
        Some(form) => {
            log::info!(
                "PDF contains an embedded form of type {:#?}",
                form.form_type()
            );

            for (key, value) in form.field_values(&pages).iter() {
                log::info!("{:?} => {:?}", key, value);
            }
        }
        None => log::info!("PDF does not contain an embedded form"),
    };

    // Report labels, boundaries, and metrics for each page to the console.

    pages.iter().enumerate().for_each(|(page_index, page)| {
        if let Some(label) = page.label() {
            log::info!("Page {} has a label: {}", page_index, label);
        }

        log::info!(
            "Page {} width: {}, height: {}",
            page_index,
            page.width().value,
            page.height().value
        );

        for boundary in page.boundaries().iter() {
            log::info!(
                "Page {} has defined {:#?} box ({}, {}) - ({}, {})",
                page_index,
                boundary.box_type,
                boundary.bounds.left.value,
                boundary.bounds.top.value,
                boundary.bounds.right.value,
                boundary.bounds.bottom.value,
            );
        }

        log::info!(
            "Page {} has paper size {:#?}",
            page_index,
            page.paper_size()
        );

        for (link_index, link) in page.links().iter().enumerate() {
            log::info!(
                "Page {} link {} has action of type {:?}",
                page_index,
                link_index,
                link.action().map(|action| action.action_type())
            );

            // For links that have URI actions, output the destination URI.

            if let Some(action) = link.action() {
                if let Some(uri_action) = action.as_uri_action() {
                    log::info!("Link URI destination: {:#?}", uri_action.uri())
                }
            }
        }

        let text = page.text().unwrap();

        for (annotation_index, annotation) in page.annotations().iter().enumerate() {
            log::info!(
                "Page {} annotation {} has text: {:?}, bounds: {:?}",
                page_index,
                annotation_index,
                text.for_annotation(&annotation),
                annotation.bounds()
            );
        }
    });
}

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([300.0, 220.0])
            .with_icon(
                // NOTE: Adding an icon is optional
                eframe::icon_data::from_png_bytes(&include_bytes!("../assets/icon-256.png")[..])
                    .expect("Failed to load icon"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "eframe template",
        native_options,
        Box::new(|cc| Ok(Box::new(credit_card_billsplit::TemplateApp::new(cc)))),
    )
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    // Redirect `log` message to `console.log` and friends:
    //eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let start_result = eframe::WebRunner::new()
            .start(
                "the_canvas_id",
                web_options,
                Box::new(|cc| Ok(Box::new(credit_card_billsplit::TemplateApp::new(cc)))),
            )
            .await;

        // Remove the loading text and spinner:
        let loading_text = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("loading_text"));
        if let Some(loading_text) = loading_text {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}
