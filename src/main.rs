//! Forgia V2 — Root binary entry-point (Renzora-style).
//!
//! `cargo run` from workspace root launches the game. Real assembly lives in
//! the `forgia-game` library crate.

fn main() {
    forgia_game::run_game();
}
