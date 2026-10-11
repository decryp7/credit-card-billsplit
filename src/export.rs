//! Saving a split to a file and loading it back, plus a CSV export for spreadsheets.

use crate::bill_reader::{last_four, Transaction};
use crate::merchant::Merchant;

/// Marks a JSON file as a split saved by this app.
const APP: &str = "credit-card-billsplit";
const VERSION: u32 = 1;

/// A split saved to a file: the transactions and the card each one is assigned to.
#[derive(serde::Deserialize, serde::Serialize)]
pub struct SavedSplit {
    pub app: String,
    pub version: u32,
    /// Name of the statement the transactions were read from.
    pub statement: Option<String>,
    pub transactions: Vec<Transaction>,
}

/// Whether a file name looks like a saved split rather than a statement.
pub fn is_saved_split(file_name: &str) -> bool {
    file_name.to_lowercase().ends_with(".json")
}

/// Name for an exported file, based on the statement's: "eStatement.pdf" -> "eStatement.split.json".
pub fn export_name(statement: Option<&str>, extension: &str) -> String {
    let stem = statement
        .map(|name| name.rsplit_once('.').map_or(name, |(stem, _)| stem))
        .map(|stem| stem.trim_end_matches(".split"))
        .filter(|stem| !stem.is_empty())
        .unwrap_or("billsplit");
    format!("{stem}.split.{extension}")
}

pub fn to_json(statement: Option<&str>, transactions: &[Transaction]) -> Vec<u8> {
    let split = SavedSplit {
        app: APP.to_string(),
        version: VERSION,
        statement: statement.map(str::to_string),
        transactions: transactions.to_vec(),
    };
    serde_json::to_vec_pretty(&split).expect("a split can always be serialized")
}

pub fn from_json(data: &[u8]) -> Result<SavedSplit, String> {
    let split: SavedSplit = serde_json::from_slice(data)
        .map_err(|e| format!("This doesn't look like a split saved by this app ({e})."))?;
    if split.app != APP {
        return Err("This doesn't look like a split saved by this app.".to_string());
    }
    if split.version > VERSION {
        return Err("This split was saved by a newer version of the app. Reload the page to update.".to_string());
    }
    Ok(split)
}

/// One row per transaction, with what was worked out about the merchant.
pub fn to_csv(transactions: &[Transaction], merchants: &[Merchant]) -> Vec<u8> {
    // The byte order mark makes Excel read the file as UTF-8.
    let mut csv = String::from("\u{feff}Date,Description,Merchant,Category,Place,Amount (SGD),Card,Assigned to\r\n");
    for (transaction, merchant) in transactions.iter().zip(merchants) {
        let row = [
            transaction.date.as_str(),
            transaction.description.as_str(),
            merchant.name.as_str(),
            merchant.category.label(),
            merchant.place.as_deref().unwrap_or(""),
            &format!("{:.2}", transaction.amount),
            last_four(&transaction.card),
            transaction.assigned_to.as_deref().unwrap_or(""),
        ];
        csv.push_str(&row.map(csv_field).join(","));
        csv.push_str("\r\n");
    }
    csv.into_bytes()
}

/// Quotes a field if it contains a separator, quote or line break.
fn csv_field(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Saves `data` under a name the user picks. Returns `Ok(())` if they cancel.
#[cfg(not(target_arch = "wasm32"))]
pub fn save_file(file_name: &str, filter: (&str, &[&str]), _mime: &str, data: &[u8]) -> Result<(), String> {
    let Some(path) = rfd::FileDialog::new().set_file_name(file_name).add_filter(filter.0, filter.1).save_file() else {
        return Ok(());
    };
    std::fs::write(&path, data).map_err(|e| format!("Unable to save {}: {e}", path.display()))
}

/// Hands `data` to the browser as a download.
#[cfg(target_arch = "wasm32")]
pub fn save_file(file_name: &str, _filter: (&str, &[&str]), mime: &str, data: &[u8]) -> Result<(), String> {
    use wasm_bindgen::JsCast as _;

    let fail = |e| format!("Unable to download {file_name}: {e:?}");
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(data));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(mime);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options).map_err(fail)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(fail)?;

    let document = web_sys::window().and_then(|w| w.document()).ok_or("No document")?;
    let link: web_sys::HtmlAnchorElement = document.create_element("a").map_err(fail)?
        .dyn_into().map_err(|_| "Unable to create a download link")?;
    link.set_href(&url);
    link.set_download(file_name);
    link.click();
    // The URL is left alive: revoking it right after the click can cancel the download in some
    // browsers, and it is freed when the page is closed.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merchant::identify;

    fn sample() -> Vec<Transaction> {
        let mut t = vec![
            Transaction::new("10 SEP".into(), "UNIQLO Europe LONDON GB".into(), 5.22, "1111 2222 3333 4444".into()),
            Transaction::new("11 SEP".into(), "EAT, \"DRINK\" SINGAPORE SG".into(), -8.0, "5555 6666 7777 8888".into()),
        ];
        t[1].assigned_to = None;
        t
    }

    #[test]
    fn json_round_trips() {
        let transactions = sample();
        let split = from_json(&to_json(Some("eStatement.pdf"), &transactions)).unwrap();
        assert_eq!(split.statement.as_deref(), Some("eStatement.pdf"));
        assert_eq!(split.transactions, transactions);
    }

    #[test]
    fn rejects_other_json() {
        assert!(from_json(br#"{"hello": "world"}"#).is_err());
        assert!(from_json(br#"{"app": "other", "version": 1, "statement": null, "transactions": []}"#).is_err());
    }

    #[test]
    fn writes_csv() {
        let transactions = sample();
        let merchants: Vec<Merchant> = transactions.iter().map(|t| identify(&t.description)).collect();
        let csv = String::from_utf8(to_csv(&transactions, &merchants)).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1], "10 SEP,UNIQLO Europe LONDON GB,Uniqlo,Shopping,\"London, United Kingdom\",5.22,4444,4444");
        assert!(lines[2].starts_with("11 SEP,\"EAT, \"\"DRINK\"\" SINGAPORE SG\""));
        assert!(lines[2].ends_with(",-8.00,8888,"));
    }

    #[test]
    fn names_exports_after_the_statement() {
        assert_eq!(export_name(Some("eStatement_Oct2026.pdf"), "csv"), "eStatement_Oct2026.split.csv");
        assert_eq!(export_name(Some("eStatement_Oct2026.split.json"), "json"), "eStatement_Oct2026.split.json");
        assert_eq!(export_name(None, "json"), "billsplit.split.json");
    }
}
