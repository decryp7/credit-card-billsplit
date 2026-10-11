#![warn(clippy::all, rust_2018_idioms)]

mod app;
mod bill_reader;
mod export;
mod merchant;
mod theme;

pub use app::BillSplitApp;
