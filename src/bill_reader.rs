use std::fmt::{Display, Formatter};
use std::ops::Index;
use std::time::SystemTime;
use log::{log, Level};
use pdfium_render::prelude::{PdfPageText, Pdfium, PdfiumError};
use regex::{Regex, RegexBuilder};

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Transaction {
    pub date: String,
    pub description: String,
    pub amount: f64,
    pub card: String,
    pub tags: Vec<String>
}

impl Transaction {
    pub fn new(date: String, description: String, amount: f64, card: String, tags: Vec<String>) -> Self {
        Self { date, description, amount, card, tags}
    }
}

impl Display for Transaction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}     {} {} ({}) [{}]",
            self.date,
        self.description,
        self.amount,
        self.card,
        self.tags.join(","))
    }
}

pub trait BillReader {
    fn read(&self, data: Vec<u8>) -> Vec<Transaction>;
}

pub struct CreditCardBillReader {
    pdf_reader: Pdfium,
    card_regex: Regex,
    transaction_regex: Regex,
}

impl CreditCardBillReader {
    pub fn default() -> Self {
        Self {
            pdf_reader: Pdfium::default(),
            //CITI PREMIERMILES CARD 4*** **** **** 5136 - TEO KOK YONG
            card_regex: RegexBuilder::new(r".* CARD (\d{4} \d{4} \d{4} \d{4}) - .*")
                .case_insensitive(true)
                .build().unwrap(),
            //05 JUN TAOBAO.COM Singapore SG (3.85)
            transaction_regex: RegexBuilder::new(r"(\d{2} [a-z]{3}) (.*) (\(?\d*\.\d{2}\)?)")
                .case_insensitive(true)
                .build().unwrap()
        }
    }
}

impl BillReader for CreditCardBillReader {
    fn read(&self, data: Vec<u8>) -> Vec<Transaction> {
        let mut transactions = Vec::<Transaction>::new();
        match self.pdf_reader.load_pdf_from_byte_vec(data, None) {
            Ok(d) => {
                //log!(Level::Info, "Pages: {}", d.pages().len());
                let mut card = String::default();
                d.pages().iter()
                    .enumerate()
                    .for_each(|(index, page)|{
                        match page.text() {
                            Ok(t) => {
                                for l in t.all().lines() {
                                    //log!(Level::Info, "{}", l);
                                    let card_captures = self.card_regex.captures(l)
                                        .and_then(|c| {
                                            //log!(Level::Info, "{:?}", c);
                                            if c.len() == 2 {
                                                card = c.index(1).parse().unwrap();
                                                //log!(Level::Info, "{}", card);
                                            }
                                            Some(true)
                                        });
                                    if card_captures.is_some() {
                                        continue;
                                    }

                                    let transaction_captures = self.transaction_regex.captures(l)
                                        .and_then(|c|{
                                            //log!(Level::Info, "{:?}", c);
                                            if c.len() == 4 {
                                                let mut amount_str : String = c.index(3).parse().unwrap();
                                                let mut amount = 0.0;
                                                if amount_str.starts_with("(") &&
                                                    amount_str.ends_with(")") {
                                                    amount_str = amount_str.replace("(", "");
                                                    amount_str = amount_str.replace(")", "");
                                                    amount = -(amount_str.parse::<f64>().unwrap());
                                                }else{
                                                    amount = amount_str.parse::<f64>().unwrap();
                                                }

                                                let transaction = Transaction::new(
                                                    c.index(1).parse().unwrap(),
                                                    c.index(2).parse().unwrap(),
                                                    amount,
                                                    card.clone(),
                                                    Vec::<String>::new());
                                                //log!(Level::Info, "{:?}", transaction);
                                                return Some(transaction)
                                            }
                                            None
                                        });
                                    if transaction_captures.is_some() {
                                        transactions.push(transaction_captures.unwrap());
                                    }

                                }
                            }
                            Err(_) => {}
                        }
                        //log!(Level::Info, "{}", page.text().unwrap().all());
                    });
            }
            Err(e) => {
                log!(Level::Error, "{}", e.to_string());
            }
        };

        transactions
    }
}



