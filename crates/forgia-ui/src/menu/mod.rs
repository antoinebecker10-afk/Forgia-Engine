//! Le shell neutre du menu — ce qui reste après que le hub roguelite soit
//! parti dans `forgia-menu-hub` (story-694, incrément 5).
//!
//! Deux responsabilités seulement : le curseur (capture/libération et les
//! réconciliateurs) et le shell proprement dit (caméra 2D, échelle UI, handler
//! ESC unique, pause du temps virtuel).

pub(crate) mod cursor;
pub(crate) mod shell;
