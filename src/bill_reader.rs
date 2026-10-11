use std::fmt::{Display, Formatter};
use std::sync::OnceLock;

use pdfium_render::prelude::Pdfium;
use regex::{Regex, RegexBuilder};

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Transaction {
    pub date: String,
    pub description: String,
    pub amount: f64,
    /// Full card number as printed on the statement, e.g. "1111 2222 3333 4444".
    pub card: String,
    /// Last 4 digits of the card this transaction is assigned to when splitting the bill.
    /// Defaults to the card the transaction was made on; `None` means unassigned.
    pub assigned_to: Option<String>,
}

impl Transaction {
    pub fn new(date: String, description: String, amount: f64, card: String) -> Self {
        let assigned_to = Some(last_four(&card).to_string());
        Self { date, description, amount, card, assigned_to }
    }
}

impl Display for Transaction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}     {} {} ({}) [{}]",
            self.date,
            self.description,
            self.amount,
            self.card,
            self.assigned_to.as_deref().unwrap_or("-"))
    }
}

/// Last 4 digits of a card number, e.g. "1111 2222 3333 4444" -> "4444".
pub fn last_four(card: &str) -> &str {
    card.get(card.len().saturating_sub(4)..).unwrap_or(card)
}

/// Parses "1,234.56" as 1234.56 and "(1,234.56)" (a credit) as -1234.56.
fn parse_amount(amount_str: &str) -> Option<f64> {
    let amount = amount_str.replace(['(', ')', ','], "").parse::<f64>().ok()?;
    if amount_str.starts_with('(') && amount_str.ends_with(')') { Some(-amount) } else { Some(amount) }
}

pub trait BillReader {
    fn read(&self, data: Vec<u8>) -> Result<Vec<Transaction>, String>;
}

/// Extracts transactions from the text of a Citibank credit card eStatement.
///
/// The statement lists the previous balance and payments first, followed by one section per
/// card, each starting with a "<CARD NAME> <card number> - <CARDHOLDER>" line.
pub struct StatementParser {
    card_regex: Regex,
    account_regex: Regex,
    previous_balance_regex: Regex,
    transaction_regex: Regex,
}

impl Default for StatementParser {
    fn default() -> Self {
        let build = |pattern: &str| RegexBuilder::new(pattern)
            .case_insensitive(true)
            .build()
            .unwrap();
        Self {
            //CITI PREMIERMILES WORLD MASTER **** **** **** **** - *** *** ****
            card_regex: build(r".* (\d{4} \d{4} \d{4} \d{4}) - .*"),
            //CITI PREMIERMILES WORLD MASTER **** **** **** **** Payment Due Date: ********
            account_regex: build(r".* (\d{4} \d{4} \d{4} \d{4}) Payment Due Date.*"),
            //BALANCE PREVIOUS STATEMENT 1,000.00
            previous_balance_regex: build(r"^BALANCE PREVIOUS STATEMENT (\(?[0-9,]*\.\d{2}\)?)"),
            //05 JUN ********** Singapore SG (3.85)
            transaction_regex: build(r"^(\d{2} [a-z]{3}) (.*) (\(?[0-9,]*\.\d{2}\)?)$"),
        }
    }
}

impl StatementParser {
    pub fn parse<'a>(&self, lines: impl IntoIterator<Item = &'a str>) -> Vec<Transaction> {
        let mut transactions = Vec::new();
        // Card of the section currently being read.
        let mut card: Option<String> = None;
        // The previous balance and payments are listed before the first card section and
        // belong to the primary account, whose number is shown in the page header.
        let mut account: Option<String> = None;

        for line in lines {
            let line = line.trim();

            if let Some(c) = self.card_regex.captures(line) {
                card = Some(c[1].to_string());
                continue;
            }

            if card.is_none() && account.is_none() {
                if let Some(c) = self.account_regex.captures(line) {
                    account = Some(c[1].to_string());
                }
            }

            let Some(current_card) = card.as_ref().or(account.as_ref()) else {
                continue;
            };

            if let Some(c) = self.previous_balance_regex.captures(line) {
                if let Some(amount) = parse_amount(&c[1]) {
                    transactions.push(Transaction::new(
                        String::default(),
                        "BALANCE PREVIOUS STATEMENT".to_string(),
                        amount,
                        current_card.clone()));
                }
                continue;
            }

            if let Some(c) = self.transaction_regex.captures(line) {
                if let Some(amount) = parse_amount(&c[3]) {
                    transactions.push(Transaction::new(
                        c[1].to_string(),
                        c[2].to_string(),
                        amount,
                        current_card.clone()));
                }
            }
        }

        transactions
    }
}

/// Pdfium can only be bound once per process, so the bindings are shared by all readers.
static PDFIUM: OnceLock<Pdfium> = OnceLock::new();

/// Reads transactions from a Citibank credit card eStatement PDF.
pub struct CreditCardBillReader {
    pdfium: &'static Pdfium,
    parser: StatementParser,
}

impl CreditCardBillReader {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            pdfium: Self::pdfium()?,
            parser: StatementParser::default(),
        })
    }

    fn pdfium() -> Result<&'static Pdfium, String> {
        if let Some(pdfium) = PDFIUM.get() {
            return Ok(pdfium);
        }
        let pdfium = Self::bind_pdfium()?;
        Ok(PDFIUM.get_or_init(|| pdfium))
    }

    /// On the web, Pdfium is the WASM module initialised by index.html.
    #[cfg(target_arch = "wasm32")]
    fn bind_pdfium() -> Result<Pdfium, String> {
        Ok(Pdfium::default())
    }

    /// Natively, use a Pdfium library next to the app, falling back to one installed on the system.
    #[cfg(not(target_arch = "wasm32"))]
    fn bind_pdfium() -> Result<Pdfium, String> {
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
            .or_else(|_| Pdfium::bind_to_system_library())
            .map(Pdfium::new)
            .map_err(|e| format!("Unable to load the Pdfium library: {e}"))
    }
}

impl BillReader for CreditCardBillReader {
    fn read(&self, data: Vec<u8>) -> Result<Vec<Transaction>, String> {
        let document = self.pdfium.load_pdf_from_byte_vec(data, None)
            .map_err(|e| format!("Unable to open the PDF: {e}"))?;

        let text: String = document.pages().iter()
            .filter_map(|page| page.text().ok().map(|t| t.all()))
            .collect::<Vec<_>>()
            .join("\n");

        let transactions = self.parser.parse(text.lines());
        transactions.iter().for_each(|t| log::debug!("{t}"));
        Ok(transactions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATEMENT: &str = "\
CITI PREMIERMILES WORLD MASTER 0.00 0.00 0.00 CITI MILES 0
1111222233334444 0.00 0.00
CITI PREMIERMILES WORLD MASTER 1111 2222 3333 4444 Payment Due Date: January 01, 2026
0.00 0.00 0.00 0.00 0.00 0.00
BALANCE PREVIOUS STATEMENT 1,000.00
14 SEP MONEYSEND JOHN DOE SINGAPORE SG (400.00)
14 SEP MONEYSEND JOHN DOE SINGAPORE SG (300.00)
26 SEP MONEYSEND JOHN DOE SINGAPORE SG (200.00)
SUB-TOTAL: (900.00)
CITI PREMIERMILES WORLD MASTER 1111 2222 3333 4444 - JOHN DOE
10 SEP ONLINE STORE LONDON GB 10.00
FOREIGN AMOUNT U.S. DOLLAR 7.50
11 SEP SPORTS CLUB Singapore SG 100.00
XXXX-XXXX-XXXX-1234
14 SEP CCY CONVERSION FEE SGD 100.00 1.00
24 SEP MARKETPLACE SINGAPORE SG 50.00
25 SEP ONLINE SHOP SEATTLE US (20.00)
SUB-TOTAL: 141.00
CITI PREMIERMILES WORLD MASTER 5555 6666 7777 8888 - JANE DOE
11 SEP RESTAURANT SINGAPORE SG 30.00
CITI PREMIERMILES WORLD MASTER 1111 2222 3333 4444 Payment Due Date: January 01, 2026
26 SEP CAFE SINGAPORE SG 12.50
SUB-TOTAL: 42.50
GRAND TOTAL 283.50";

    fn total_for(transactions: &[Transaction], card: &str) -> f64 {
        let total: f64 = transactions.iter()
            .filter(|t| t.assigned_to.as_deref() == Some(card))
            .map(|t| t.amount)
            .sum();
        (total * 100.0).round() / 100.0
    }

    #[test]
    fn parses_amounts() {
        assert_eq!(parse_amount("1,234.56"), Some(1234.56));
        assert_eq!(parse_amount("(1,234.56)"), Some(-1234.56));
        assert_eq!(parse_amount("abc"), None);
    }

    #[test]
    fn last_four_digits() {
        assert_eq!(last_four("1111 2222 3333 4444"), "4444");
        assert_eq!(last_four("12"), "12");
    }

    #[test]
    fn includes_previous_balance_and_payments() {
        let transactions = StatementParser::default().parse(STATEMENT.lines());

        assert_eq!(transactions[0].description, "BALANCE PREVIOUS STATEMENT");
        assert_eq!(transactions[0].card, "1111 2222 3333 4444");
        assert_eq!(transactions[3].amount, -200.0);

        let total: f64 = transactions.iter().map(|t| t.amount).sum();
        assert_eq!((total * 100.0).round() / 100.0, 283.5);
    }

    #[test]
    fn assigns_transactions_to_their_card() {
        let transactions = StatementParser::default().parse(STATEMENT.lines());

        assert_eq!(transactions.len(), 11);
        assert_eq!(total_for(&transactions, "4444"), 241.0);
        assert_eq!(total_for(&transactions, "8888"), 42.5);
        // The page header must not switch back to the primary card mid-section.
        assert_eq!(transactions.last().unwrap().card, "5555 6666 7777 8888");
    }

    #[test]
    fn uses_last_amount_on_line() {
        let transactions = StatementParser::default().parse(STATEMENT.lines());
        let fee = transactions.iter().find(|t| t.description.starts_with("CCY")).unwrap();
        assert_eq!(fee.description, "CCY CONVERSION FEE SGD 100.00");
        assert_eq!(fee.amount, 1.00);
    }
}
