//! Story-464 — Tests headless du gate LOS sur la state machine bot.
//!
//! Valide que `decide_bot_state` (fonction pure exposée) downgrade Chase en
//! Idle quand le bot perd `has_los` et que `los_lost_grace_left` expire, mais
//! garde Chase tant que `alerted` ou la grace est active.

use forgia_ai_arena_bot::{decide_bot_state, ArenaBot, BotState};

fn bot_with(has_los: bool, los_lost_grace_left: f32, alerted: bool) -> ArenaBot {
    let mut bot = ArenaBot::default();
    bot.has_los = has_los;
    bot.los_lost_grace_left = los_lost_grace_left;
    bot.alerted = alerted;
    // Default ArenaBot : detect_range=25, stop_distance=1.5 — assez pour les
    // assertions ci-dessous où on travaille à dist=10 (Chase zone).
    bot
}

#[test]
fn chase_kept_when_los_present() {
    // Bot a vue claire + distance dans Chase zone → Chase.
    let bot = bot_with(true, 0.0, false);
    assert_eq!(decide_bot_state(&bot, 10.0), BotState::Chase);
}

#[test]
fn chase_kept_during_los_lost_grace() {
    // Bot a perdu LOS mais grace encore active → Chase autorisé (last sight memory).
    let bot = bot_with(false, 1.5, false);
    assert_eq!(decide_bot_state(&bot, 10.0), BotState::Chase);
}

#[test]
fn lost_los_drops_chase_after_grace() {
    // Bot a perdu LOS + grace expirée + pas alerté → Idle (drop tracking).
    // C'est le scénario user "tracking permanent" — doit être éliminé.
    let bot = bot_with(false, 0.0, false);
    assert_eq!(decide_bot_state(&bot, 10.0), BotState::Idle);
}

#[test]
fn keeps_chase_if_alerted_without_los() {
    // Bot a perdu LOS + grace expirée MAIS alerted=true (audio AI) → Chase forcé.
    // Couvre le scénario "player a tiré, bot cherche même sans vue".
    let bot = bot_with(false, 0.0, true);
    assert_eq!(decide_bot_state(&bot, 10.0), BotState::Chase);
}

#[test]
fn regains_chase_on_los_reacquire() {
    // Scénario : bot était Idle (LOS perdu, grace expirée, pas alerté).
    // Re-acquisition LOS (has_los=true) doit re-déclencher Chase immédiatement.
    let bot = bot_with(true, 0.0, false);
    assert_eq!(decide_bot_state(&bot, 10.0), BotState::Chase);
}

#[test]
fn attack_requires_pursue_gate() {
    // Bot dans stop_distance MAIS sans LOS ni alert ni grace → Idle.
    // Sans le gate Story-464, un bot collé au player à travers un mur serait
    // en Attack (donc tirerait, mais bot_shoot_at_target gate has_los).
    // Avec le gate, il passe en Idle = comportement cohérent visuellement.
    let bot = bot_with(false, 0.0, false);
    assert_eq!(decide_bot_state(&bot, 1.0), BotState::Idle);
}

#[test]
fn attack_when_close_with_sight() {
    // Bot dans stop_distance + LOS → Attack (cas nominal combat rapproché).
    let bot = bot_with(true, 0.0, false);
    assert_eq!(decide_bot_state(&bot, 1.0), BotState::Attack);
}

#[test]
fn idle_when_out_of_detect_range_even_with_los() {
    // Bot voit le player mais hors detect_range → Idle.
    // Pas une régression : detect_range est la limite de perception.
    let bot = bot_with(true, 0.0, false);
    assert_eq!(decide_bot_state(&bot, 100.0), BotState::Idle);
}

#[test]
fn spawn_grace_allows_initial_chase() {
    // ArenaBot::default() init `los_lost_grace_left = 2.0` (grace spawn).
    // Sans cette grace, les bots gèleraient jusqu'au 1er raycast LOS (~125ms).
    let bot = ArenaBot::default();
    assert!(bot.los_lost_grace_left > 0.0,
        "default ArenaBot doit avoir grace de spawn pour bootstrap");
    // À distance Chase, le bot peut Chase dès spawn même sans 1er LOS check.
    assert_eq!(decide_bot_state(&bot, 10.0), BotState::Chase);
}
