use log::{log, Level};
use pdfium_render::prelude::Pdfium;

struct Transaction {
    description: String,
    amount: f32,
    tags: Vec<String>
}

impl Transaction {
    pub fn new(description: String, amount: f32, tags: Vec<String>) -> Self {
        Self { description, amount, tags}
    }
}

pub trait BillReader {
    fn read(&self, data: Vec<u8>) -> Vec<Transaction>;
}

struct CreditCardBillReader {
    pdf_reader: Pdfium
}

impl CreditCardBillReader {
    pub fn default() -> Self {
        Self { pdf_reader: Pdfium::default() }
    }
}

impl BillReader for CreditCardBillReader {
    fn read(&self, data: Vec<u8>) -> Vec<Transaction> {
        let mut transactions = Vec::<Transaction>::new();
        match self.pdf_reader.load_pdf_from_byte_vec(data, None) {
            Ok(d) => {
                log!(Level::Info, "Pages: {}", d.pages().len());
                d.pages().iter()
                    .enumerate()
                    .for_each(|(index, page)|{
                        log!(Level::Info, "{}", page.text().unwrap().all());
                    });
            }
            Err(e) => {
                log!(Level::Error, "{}", e.to_string());
            }
        };

        transactions
    }
}



