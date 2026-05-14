//! Thin binary wrapper. Real assembly lives in `lib.rs`.
//! Kept for backward-compat (`cargo run -p forgia-game`). Prefer root `cargo run`.

fn main() {
    forgia_game::run_game();
}
