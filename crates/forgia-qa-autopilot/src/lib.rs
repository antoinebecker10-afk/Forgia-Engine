//! Forgia QA Autopilot — bots automatisés QA (P6 story-391).
//!
//! Trois bots progressifs :
//!
//! - **`SmokeBot`** (W1) : run TestApp scénarios + assertions BugBus.
//!   Cas d'usage : CI sur chaque PR.
//! - **`SoakBot`** (W3) : long-run stability détectant leaks/drifts.
//! - **LLM ExplorerBot** (post-P6) : LLM-driven exploration adaptive
//!   (TITAN-inspired arxiv 2509.22170). Différé.
//!
//! ## Quick start
//!
//! ```ignore
//! use forgia_qa_autopilot::{Bot, SmokeBot};
//!
//! let mut bot = SmokeBot::new("crash_smoke", |test| {
//!     test.tick(60);
//!     test.expect_no_blocker();
//! });
//! let result = bot.run();
//! match result {
//!     forgia_qa_autopilot::BotResult::Pass { stats } => {
//!         println!("OK : {} frames, {} bugs", stats.frames_consumed, stats.bugs_emitted);
//!     }
//!     forgia_qa_autopilot::BotResult::Fail { reason, .. } => {
//!         eprintln!("FAIL : {reason}");
//!     }
//!     forgia_qa_autopilot::BotResult::Inconclusive { note } => {
//!         eprintln!("INCONCLUSIVE : {note}");
//!     }
//! }
//! ```
//!
//! Cf. `docs/qa/PLAN_P6.md` pour plan complet.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod bot;
pub mod runner;
pub mod smoke;
pub mod soak;

pub use bot::{Bot, BotResult, BotStats};
pub use runner::{BotRunner, RunEntry, RunReport, RunSummary};
pub use smoke::SmokeBot;
pub use soak::SoakBot;
