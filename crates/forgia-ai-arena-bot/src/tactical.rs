//! # Tactical AI — Phase 2-4 story-456 (2026-05-18)
//!
//! Patterns AAA research sourcés :
//! - **LOS check** ~8 Hz (Halo 2 props poll, Damian Isla GDC 2005)
//! - **Strafing** sin + Perlin-like noise (Doom 2016 imp dodge — anti-prévisibilité)
//! - **Context steering simplifié** 3-ray forward+sides (Andrew Fray Game AI Pro 2 §18)
//! - **Reaction time grace** 350ms warmup post-LOS (humain casual 200-300ms)
//! - **Gunshot alert radius** 25m, +600ms grace si alerted-not-yet-seen
//!
//! Tous params dans `TacticalTuning` Resource (genome-driven via consumer crate).

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_combat::prelude::CombatHitEvent;

use crate::{ArenaBot, BotState, BotTarget};

// ─── Tuning Resource (genome-driven) ───────────────────────────────────

#[derive(Resource, Debug, Clone, Copy)]
pub struct TacticalTuning {
    /// Fréquence raycast LOS bot→player (Hz). 8Hz = 7.5 frames @60fps. Anti CPU spam.
    pub los_check_hz: f32,
    /// Grace window post-acquisition LOS avant 1er tir (sec). AAA reaction time.
    pub los_grace_secs: f32,
    /// Amplitude latérale strafe (m). Doom imp dodge style.
    pub strafe_amplitude_m: f32,
    /// Fréquence sinusoid strafe (Hz). 0.9 = période ~1.1s.
    pub strafe_freq_hz: f32,
    /// Poids du noise additif sur strafe (0..1). 0 = pure sin (prévisible), 1 = full noise.
    pub strafe_noise_weight: f32,
    /// Distance raycast obstacle avoidance (m).
    pub local_avoid_dist_m: f32,
    /// Rayon de perception "tir player entendu" — bots dans ce rayon → alerted.
    pub gunshot_alert_radius_m: f32,
    /// Grace window supplémentaire post-alert avant 1er tir (sec). AAA "look around" feel.
    pub gunshot_alert_los_grace_secs: f32,
    /// Durée du flag alerted (forced Chase even out of detect_range).
    pub alert_duration_secs: f32,
    /// Durée pendant laquelle le bot reste autorisé à Chase après avoir perdu LOS.
    ///
    /// # 2026-08-13 — 2,0 → 6,0 s, et le chiffre est emprunté, pas choisi
    ///
    /// Rapporté en jeu : *« quand je quitte un mob, il ne peut pas faire tout le tour
    /// de l'arène ? »*. Non — il oubliait en 2 s.
    ///
    /// **Ni Minecraft ni World of Warcraft ne lâchent sur la vue.** MC borne le suivi
    /// par `generic.follow_range` (16 blocs, ~40 pour un zombie) ; WoW par une **laisse**
    /// en distance, plus un **evade après ~6 s sans combat actif** en retail. Forgia était
    /// le seul des trois à utiliser un chronomètre de *vue* — d'où l'impression d'un
    /// ennemi amnésique.
    ///
    /// 6,0 s reprend le compteur d'evade de WoW. **Et le modèle Minecraft a été écarté
    /// exprès** : un zombie est bien plus lent qu'un joueur, donc la fuite par la
    /// distance y fonctionne. Ici un grunt file à **9,0 m/s** contre **9,75 m/s** en
    /// sprint — 92 % de ta vitesse. Gagner 20 m d'écart demanderait 27 s de sprint pur :
    /// **on ne sème pas un grunt à la course.** Un `follow_range` à la Minecraft rendrait
    /// donc la fuite impossible ; c'est le chronomètre qui doit la porter.
    pub los_lost_grace_secs: f32,
    /// Distance au-delà de laquelle le bot abandonne **immédiatement**, vue ou pas.
    ///
    /// La *laisse* de WoW. Ce n'est PAS la mécanique de fuite — vu l'écart de vitesse
    /// ci-dessus, un joueur ne l'atteint quasiment jamais en courant. C'est un **filet
    /// de sécurité** : il empêche un mob de traverser toute l'arène derrière un joueur
    /// qui l'aurait accroché à l'autre bout, et garantit qu'aucun ennemi ne s'égare
    /// hors de la zone de combat.
    ///
    /// 2 × `detect_range` : on acquiert à 25 m, on retient jusqu'à 50. **Séparer les
    /// deux est ce que font les deux références** — WoW aggro ~20 yd mais laisse bien
    /// plus longue ; MC zombie `follow_range` 40 contre 16 pour la plupart.
    pub chase_leash_m: f32,
    /// Période d'écriture sensor `forgia_bot_ai.json` (sec).
    pub sensor_period_secs: f32,

    // ── Suivi de sol (story-685) ────────────────────────────────────────────
    /// Marche maximale que le bot peut MONTER (m).
    ///
    /// 0,45 m = `MaxStepHeight` d'Unreal, la valeur que nos patterns de carte
    /// citent déjà. Au-delà, c'est une paroi : le bot doit la contourner, pas
    /// l'escalader — il n'a ni saut ni escalade.
    pub max_step_up_m: f32,
    /// Dénivelé maximum que le bot accepte de DESCENDRE en un pas (m).
    ///
    /// Plus large que la montée : descendre une marche est toujours plus facile
    /// que la gravir. Au-delà, c'est un vide — le bot refuse d'avancer plutôt
    /// que de tomber, faute de quoi il quitterait l'arène par un rebord.
    pub max_step_down_m: f32,
    /// Hauteur au-dessus du bot d'où part le rayon vers le sol (m). Doit
    /// dépasser `max_step_up_m`, sinon une marche montante ne serait jamais vue.
    pub ground_probe_height_m: f32,

    // ── Traversée d'exception (2026-08-13) ──────────────────────────────────
    /// Durée de blocage, EN POURSUITE, après laquelle le bot traverse l'obstacle.
    ///
    /// C'est un **filet**, pas une mécanique. Il rend le jeu jouable quand la
    /// géométrie ou la navigation échoue ; il ne les répare pas. La mesure qui
    /// compte est `phase_triggered_session` : s'il monte, un défaut subsiste
    /// dessous.
    pub phase_after_secs: f32,
    /// Durée maximale d'une traversée (s). Bornée : de quoi franchir un prop,
    /// pas de quoi remonter un couloir à travers les murs.
    pub phase_max_secs: f32,
    /// Fraction du pas VOULU en dessous de laquelle on considère qu'un bot
    /// n'avance pas. Il ne suffit pas de tester « déplacement nul » : un bot
    /// qui rabote un mur en glissant avance encore un peu tout en tournant en
    /// rond. Le seuil est donc RELATIF à sa vitesse, pas absolu.
    pub stuck_progress_frac: f32,
    /// Durée de non-progression, EN POURSUITE, avant de déclencher la sortie.
    /// Assez long pour ne pas se déclencher sur un frôlement de mur, assez
    /// court pour qu'un joueur ne voie pas un ennemi planté.
    pub stuck_after_secs: f32,
    /// Durée pendant laquelle le bot longe l'obstacle au lieu de foncer vers sa
    /// cible. Il ne se téléporte JAMAIS : on ne change que sa direction.
    pub unstick_secs: f32,
    // ── Story-700 inc.3 — suivi de chemin (navmesh) ───────────────
    /// Distance à laquelle un point du chemin est considéré atteint (m). Trop
    /// petit = le bot tourne autour sans jamais valider ; trop grand = il coupe
    /// les virages et frotte les murs que le chemin contournait.
    ///
    /// 2026-08-13 — **abaissé de 1,0 à 0,35 m** après « les mobs se bloquent dans les
    /// portes ». Une porte fait 4 m, dilatée à 3 m de passage utile : valider son point
    /// à 1 m de distance faisait couper le virage en plein dans le montant. 0,35 m
    /// reste au-dessus du pas d'une frame (un grunt à 9 m/s parcourt 0,15 m à 60 fps),
    /// donc le bot ne peut pas orbiter autour du point sans jamais le valider.
    pub waypoint_arrive_m: f32,
    /// Délai minimum entre deux recalculs de chemin (s). Le chemin est du travail
    /// de PLANIFICATION, pas de frame : N bots qui retriangulent chaque frame
    /// mangeraient le budget pour un résultat identique.
    pub repath_period_secs: f32,
    /// Déplacement de la cible au-delà duquel on recalcule sans attendre le délai
    /// (m). C'est ce qui garde la poursuite réactive malgré le throttle ci-dessus.
    pub target_moved_repath_m: f32,
}

impl Default for TacticalTuning {
    fn default() -> Self {
        Self {
            los_check_hz: 8.0,
            los_grace_secs: 0.35,
            strafe_amplitude_m: 1.8,
            strafe_freq_hz: 0.9,
            strafe_noise_weight: 0.35,
            local_avoid_dist_m: 2.5,
            gunshot_alert_radius_m: 25.0,
            gunshot_alert_los_grace_secs: 0.6,
            alert_duration_secs: 4.0,
            los_lost_grace_secs: 6.0,
            chase_leash_m: 50.0,
            sensor_period_secs: 1.0,
            max_step_up_m: 0.45,
            max_step_down_m: 1.2,
            ground_probe_height_m: 1.0,
            // Reflet d'`arena_bots.toml` — c'est LUI la source de vérité, le
            // génome écrase cette Resource à chaque chargement
            // (`sync_tactical_tuning_from_genome`). Changer ces lignes SEULES
            // serait inerte : erreur commise le 2026-08-13 sur
            // `los_lost_grace_secs`, rattrapée par le capteur.
            phase_after_secs: 2.5,
            phase_max_secs: 1.0,
            stuck_progress_frac: 0.25,
            stuck_after_secs: 0.7,
            unstick_secs: 0.9,
            waypoint_arrive_m: 0.35,
            // 2 Hz : un grunt à 9 m/s parcourt 4,5 m entre deux calculs, très en deçà
            // de la maille d'une arène. `target_moved_repath_m` rattrape le reste.
            repath_period_secs: 0.5,
            target_moved_repath_m: 2.0,
        }
    }
}

// ─── Sensor state ──────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct BotAiSensor {
    pub last_write_secs: f32,
    pub bots_alive: u32,
    pub bots_with_los: u32,
    pub bots_alerted: u32,
    pub bots_chasing: u32,
    pub bots_attacking: u32,
    pub los_checks_session: u32,
    pub alerts_triggered_session: u32,
    /// Nombre de désenlisements DÉCLENCHÉS depuis le lancement.
    ///
    /// Cumulé, et non instantané : les deux premiers compteurs
    /// (`bots_unsticking` / `bots_stalling`) valent 0 dès qu'il ne reste aucun
    /// bot vivant, donc une lecture faite après une run ne peut pas dire si le
    /// chien de garde a servi. Un compteur qui ne sait répondre à sa propre
    /// question est aveugle, pas vert.
    pub unstick_triggered_session: u32,
    /// Traversées d'exception DÉCLENCHÉES depuis le lancement.
    ///
    /// ⚠️ **Ce compteur n'est pas une réussite.** Chaque incrément dit qu'un bot est
    /// resté bloqué en poursuite assez longtemps pour qu'on doive lui faire franchir
    /// un solide. Le filet rend le jeu jouable ; il ne répare rien. S'il monte, un
    /// défaut de géométrie ou de navigation subsiste dessous — croiser avec
    /// `forgia2_bot_traces.json` pour savoir lequel.
    pub phase_triggered_session: u32,
    /// Chemins trouvés depuis le lancement (recalés compris).
    pub paths_ok_session: u32,
    /// Parmi eux, ceux obtenus **après recalage d'une extrémité sur le maillage**.
    ///
    /// Le maillage abandonne une bande d'un rayon d'agent le long de chaque mur : un
    /// joueur qui s'y colle y est injoignable tel quel. Ce compteur mesure à quel point
    /// ça arrive. Quelques pour cent = normal, un joueur longe les murs. **Une majorité
    /// = le maillage ne décrit plus la zone jouable**, et c'est la géométrie qu'il faut
    /// regarder, pas le recalage.
    pub paths_snapped_session: u32,
    /// Parmi les recalages, ceux où c'est **le BOT** qui était hors du maillage.
    ///
    /// La distinction décide du remède. Le joueur hors maillage est normal : il longe
    /// les murs. Le BOT hors maillage veut dire qu'il se tient là où le maillage le lui
    /// interdit — donc que l'**emprise déclarée** d'un obstacle ne correspond pas à son
    /// **collider**. C'est la classe de défaut de `spawn-clearance.md` §4, et aucun
    /// recalage ne la corrige : elle se corrige à la source des emprises.
    pub paths_snapped_bot_session: u32,
    /// Recalculs dont le résultat a été REJETÉ par l'hystérésis.
    ///
    /// C'est la mesure de l'oscillation de route. Chaque incrément est un
    /// changement de trajet que le bot AURAIT fait et qui ne lui aurait rien
    /// rapporté — un demi-tour gratuit. Avant l'hystérésis, ces demi-tours
    /// étaient tous appliqués : quatre bots parcouraient 8 à 20 m pour 1 à 17 cm
    /// de déplacement net.
    ///
    /// Un compteur élevé n'est PAS un problème : c'est le nombre d'oscillations
    /// évitées. Il devient un signal quand il approche `paths_ok_session`, ce qui
    /// voudrait dire que le maillage hésite en permanence — géométrie ambiguë.
    pub paths_kept_session: u32,
    /// **Chemins REFUSÉS par le maillage.** Un échec renvoie le bot en ligne droite,
    /// c'est-à-dire au comportement d'avant le navmesh — donc « bloqué à cet endroit
    /// précis ». Sans ce compteur, le symptôme se devine ; avec, il se mesure.
    pub paths_failed_session: u32,
    /// (x, z) du dernier refus — position du BOT. Un compte dit *combien*, ces deux
    /// points disent *où*, et permettent de trancher entre « le bot est hors du
    /// maillage » et « la cible l'est ». Deux causes opposées, un seul compteur.
    pub last_fail_from: (f32, f32),
    /// (x, z) du dernier refus — position de la CIBLE.
    pub last_fail_to: (f32, f32),
    /// Au dernier refus, le BOT était-il hors du maillage ?
    pub last_fail_bot_off_mesh: bool,
    /// Au dernier refus, la CIBLE était-elle hors du maillage ?
    ///
    /// Les deux à `false` disent la troisième cause, et la plus grave : les deux points
    /// sont navigables mais **aucun trajet ne les relie**. C'est une poche isolée — un
    /// défaut de géométrie, pas de navigation, et aucun recalage ne le corrigera.
    pub last_fail_target_off_mesh: bool,
}

// ─── Phase 2 — LOS check ───────────────────────────────────────────────

/// Jusqu'où un bot PERÇOIT sa cible — sa vue, pas la portée de son arme.
///
/// Un bot doit pouvoir voir (donc poursuivre) bien plus loin qu'il ne peut
/// frapper : c'est toute la différence entre un ennemi de mêlée qui charge et un
/// ennemi de mêlée qui reste planté. Le tir garde sa propre borne
/// (`bot_shoot_at_target`), donc élargir la vue ne fait tirer personne plus loin.
///
/// PUR — testable sans App.
pub fn sight_range(shoot_range: f32, detect_range: f32) -> f32 {
    shoot_range.max(detect_range)
}

/// Raycast bot.shoulder → player.torso à `los_check_hz`. Set `has_los` + `los_grace_left`.
/// Filter exclut le bot lui-même via predicate (anti self-hit).
#[allow(clippy::too_many_arguments)]
pub fn bot_los_check(
    mut bots: Query<(
        Entity,
        &mut ArenaBot,
        &GlobalTransform,
        &crate::BotShootConfig,
    )>,
    targets: Query<(Entity, &GlobalTransform), With<BotTarget>>,
    rapier: ReadRapierContext,
    tuning: Res<TacticalTuning>,
    time: Res<Time>,
    mut sensor: ResMut<BotAiSensor>,
    q_child_of: Query<&ChildOf>,
    // `Local` réutilisé plutôt qu'un `Vec::new()` par frame : ce système tourne à
    // 8 Hz par bot sur un chemin déjà chargé (`scalability.md`).
    mut fratrie: Local<Vec<Entity>>,
) {
    let Some((target_entity, target_tf)) = targets.iter().next() else {
        return;
    };
    let Ok(ctx) = rapier.single() else { return };
    let dt = time.delta_secs();
    let check_interval = 1.0 / tuning.los_check_hz.max(0.1);
    let target_pos = target_tf.translation();

    // 2026-08-13 — UN BOT NE FAIT PAS DE L'OMBRE À UN AUTRE.
    //
    // Rapporté en jeu : « ils ont un périmètre d'action autour d'eux au spawn et
    // restent bloqués dedans ». Les capteurs ont tranché : 7 bots vivants, **1 seul**
    // avec ligne de vue, **1 seul** en poursuite — et aucun signalé coincé. Ils
    // n'étaient pas bloqués, ils étaient AVEUGLES, donc `Idle`, donc immobiles là où
    // ils étaient nés.
    //
    // Le rayon de perception n'excluait que le bot qui le lance. Dans une grappe,
    // celui de devant voyait le joueur et **masquait tous ceux de derrière**. Plus le
    // groupe est dense, moins il engage — l'inverse exact de ce qu'on attend d'une
    // vague.
    //
    // Les alliés ne bloquent donc plus la perception. Le TIR, lui, garde ses propres
    // gardes : voir n'est pas tirer, la distinction est déjà posée plus bas.
    fratrie.clear();
    fratrie.extend(bots.iter().map(|(e, ..)| e));

    for (bot_entity, mut bot, bot_tf, config) in &mut bots {
        if bot.state == BotState::Dead {
            continue;
        }
        // Décrémente timer + grace.
        bot.los_check_left -= dt;
        bot.los_grace_left = (bot.los_grace_left - dt).max(0.0);
        bot.alert_left = (bot.alert_left - dt).max(0.0);
        // Story-464 — décrément continu de la grace "LOS perdu" pour que Chase
        // expire même entre 2 raycasts LOS (8Hz = ~125ms entre checks).
        bot.los_lost_grace_left = (bot.los_lost_grace_left - dt).max(0.0);
        if bot.alert_left <= 0.0 {
            bot.alerted = false;
        }
        if bot.los_check_left > 0.0 {
            continue;
        }
        bot.los_check_left = check_interval;
        sensor.los_checks_session = sensor.los_checks_session.saturating_add(1);

        let origin = bot_tf.translation() + Vec3::Y * config.shoulder_y;
        let aim_at = Vec3::new(
            target_pos.x,
            target_pos.y + config.target_torso_y,
            target_pos.z,
        );
        let to_target = aim_at - origin;
        let dist = to_target.length();
        // 2026-08-04 — VOIR n'est pas TIRER.
        //
        // Cette borne était `config.range`, la portée de l'ARME. Un bot de mêlée
        // l'a à ~3 m : au-delà, `has_los` retombait à false sans même lancer le
        // rayon. Or `decide_bot_state` exige `has_los` pour passer en `Chase`,
        // et la zone d'aggro (`detect_range`) vaut 25 m. Résultat : un mob de
        // mêlée était AVEUGLE par construction dès 3 m, donc `Idle`, donc il ne
        // poursuivait jamais — « les mobs ne me traquent pas alors que je suis
        // dans leur zone d'aggro », rapporté en jeu.
        //
        // Le tir n'est pas élargi pour autant : `bot_shoot_at_target` porte son
        // PROPRE garde `dist > config.range` (lib.rs). Les deux concepts sont
        // désormais séparés — la perception porte jusqu'où le bot voit, l'arme
        // jusqu'où elle atteint.
        // LA LAISSE — au-delà, le bot abandonne SANS attendre son chronomètre. Sans
        // cette coupure nette, les 6 s de persistance laisseraient un grunt à 9 m/s
        // parcourir 54 m derrière un joueur accroché à l'autre bout de l'arène.
        if dist > tuning.chase_leash_m {
            bot.has_los = false;
            bot.los_lost_grace_left = 0.0;
            continue;
        }
        if dist < 0.5 || dist > sight_range(config.range, bot.detect_range) {
            bot.has_los = false;
            continue;
        }
        let dir = to_target / dist;
        // Story-545 (2026-05-27) — exclude_rigid_body traverse chaîne complète
        // collider→RigidBody (vs predicate root-only). Fix Roguelite enemies
        // skeleton child collider qui faisaient échouer le LOS sur self-hit.
        // Un candidat appartient-il à un ALLIÉ ? Le collider touché peut être un
        // enfant du bot (colliders de squelette), d'où la remontée de hiérarchie —
        // même précaution que la résolution de la cible juste en dessous.
        let est_un_allie = |e: Entity| -> bool {
            let mut cur = e;
            for _ in 0..5 {
                if fratrie.contains(&cur) {
                    return true;
                }
                match q_child_of.get(cur) {
                    Ok(co) => cur = co.parent(),
                    Err(_) => break,
                }
            }
            false
        };
        // `let` séparé : le prédicat doit vivre aussi longtemps que le filtre.
        let pas_un_allie = |e: Entity| !est_un_allie(e);
        let filter = QueryFilter::default()
            .exclude_rigid_body(bot_entity)
            .predicate(&pas_un_allie);
        let hit = ctx.cast_ray(origin, dir, dist, true, filter);
        let new_los = match hit {
            None => true,
            Some((hit_entity, _)) => {
                // Story-545 — walk ChildOf 4 niveaux pour résoudre target_entity
                // sur ancestor (Player root vs child collider du capsule).
                let mut current = hit_entity;
                let mut matched = current == target_entity;
                for _ in 0..4 {
                    if matched {
                        break;
                    }
                    match q_child_of.get(current) {
                        Ok(co) => {
                            current = co.parent();
                            if current == target_entity {
                                matched = true;
                            }
                        }
                        Err(_) => break,
                    }
                }
                matched
            }
        };
        // Transition false → true : démarrer grace window (reaction time AAA).
        if !bot.has_los && new_los {
            let grace = if bot.alerted {
                tuning
                    .los_grace_secs
                    .min(tuning.gunshot_alert_los_grace_secs)
            } else {
                tuning.los_grace_secs
            };
            bot.los_grace_left = grace;
        }
        // Story-464 — transition true → false : armer la grace "LOS perdu"
        // pour autoriser Chase pendant los_lost_grace_secs avant de drop en Idle.
        // Pattern AAA "last sight timer" (F.E.A.R. SAPI, Halo 2 props poll).
        if bot.has_los && !new_los {
            bot.los_lost_grace_left = tuning.los_lost_grace_secs;
        }
        // Tant que LOS est actif, on garde la grace au max (le bot voit, pas de countdown).
        if new_los {
            bot.los_lost_grace_left = tuning.los_lost_grace_secs;
        }
        bot.has_los = new_los;
    }
}

// ─── Phase 4 — Perception alert ────────────────────────────────────────

/// Consume `CombatHitEvent` filter `attacker == player` (= target reçoit damage de player).
/// Tout bot dans `gunshot_alert_radius_m` du player passe alerted=true + alert_left=duration.
/// Bot alerted force le state Chase même hors detect_range (audio-driven AI).
pub fn bot_perception_alert(
    mut events: MessageReader<CombatHitEvent>,
    mut bots: Query<(&mut ArenaBot, &GlobalTransform)>,
    q_target: Query<&GlobalTransform, With<BotTarget>>,
    tuning: Res<TacticalTuning>,
    mut sensor: ResMut<BotAiSensor>,
) {
    let Ok(target_tf) = q_target.single() else {
        // Drain events sinon ils s'accumulent.
        for _ in events.read() {}
        return;
    };
    let player_pos = target_tf.translation();
    for hit in events.read() {
        // Source du bruit = position du player tirant (proxy : si attacker = player,
        // le tir part du player). Pas le hit_world_pos (ça c'est la cible).
        let _ = hit; // dummy : ici on déclenche alert sur n'importe quel tir player.
                     // Filter : on alerte sur tous les CombatHitEvent (proxy "player a tiré").
                     // Pourrait être affiné via une dedicated WeaponFiredEvent — out of scope phase 4.
        for (mut bot, bot_tf) in &mut bots {
            if bot.state == BotState::Dead {
                continue;
            }
            let d = bot_tf.translation().distance(player_pos);
            if d <= tuning.gunshot_alert_radius_m && !bot.alerted {
                bot.alerted = true;
                bot.alert_left = tuning.alert_duration_secs;
                sensor.alerts_triggered_session = sensor.alerts_triggered_session.saturating_add(1);
            }
        }
    }
}

// ─── Phase 3 — Strafing + obstacle avoidance ──────────────────────────

/// Calcule le vecteur strafe lateral à appliquer en plus du chase forward.
/// sin(phase) bias droite/gauche, modulé par noise xorshift (anti-prévisibilité).
/// Output : direction unit dans plan XZ (perpendiculaire à to_target).
fn compute_strafe_offset(
    bot: &mut ArenaBot,
    to_target_dir: Vec3,
    tuning: &TacticalTuning,
    dt: f32,
) -> Vec3 {
    bot.strafe_phase_rad += dt * tuning.strafe_freq_hz * std::f32::consts::TAU;
    let sin = bot.strafe_phase_rad.sin();
    // xorshift32 → noise [-0.5, 0.5]
    let mut x = bot.strafe_noise_seed.max(1);
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    bot.strafe_noise_seed = x;
    let noise = (x as f32 / u32::MAX as f32) - 0.5;
    let bias = sin * (1.0 - tuning.strafe_noise_weight) + noise * 2.0 * tuning.strafe_noise_weight;
    // Right vector perpendiculaire à to_target dans plan XZ.
    let right = Vec3::new(-to_target_dir.z, 0.0, to_target_dir.x).normalize_or_zero();
    right * bias * tuning.strafe_amplitude_m
}

/// Obstacle avoidance context steering simplifié 3-ray (forward, ±45°).
/// Return : direction unit XZ ajustée pour éviter les obstacles. None si tous bloqués.
fn pick_avoid_direction(
    origin: Vec3,
    desired_dir: Vec3,
    rapier: &RapierContext,
    self_entity: Entity,
    max_dist: f32,
) -> Option<Vec3> {
    let predicate = |e: Entity| e != self_entity;
    let filter = QueryFilter::default().predicate(&predicate);
    let cast = |d: Vec3| -> f32 {
        rapier
            .cast_ray(
                origin + Vec3::Y * 0.5,
                d.normalize_or_zero(),
                max_dist,
                true,
                filter,
            )
            .map(|(_, t)| t)
            .unwrap_or(max_dist)
    };
    let right = Vec3::new(-desired_dir.z, 0.0, desired_dir.x).normalize_or_zero();
    let dir_fwd = desired_dir;
    let dir_left = (desired_dir - right * 0.7).normalize_or_zero();
    let dir_right = (desired_dir + right * 0.7).normalize_or_zero();
    let fwd = cast(dir_fwd);
    let left = cast(dir_left);
    let right_d = cast(dir_right);
    // Pick le plus dégagé (max distance).
    let best = [(fwd, dir_fwd), (left, dir_left), (right_d, dir_right)]
        .into_iter()
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))?;
    if best.0 < 0.6 {
        None // tous bloqués (< 0.6m = collision imminente)
    } else {
        Some(best.1)
    }
}

// ─── Collide-and-slide contre les murs (anti-traversée) ───────────────────────
//
// Les bots sont `RigidBody::KinematicPositionBased` déplacés par mutation directe
// de `Transform` → Rapier ne les stoppe PAS contre les murs statiques (un corps
// kinematic n'est jamais bloqué par du Fixed). De plus leurs colliders body/head
// sont des `Sensor` (story-517 : le joueur traverse les ennemis + hitscan OK).
// On valide donc chaque déplacement à la main : shapecast approximé par 3 rayons
// (centre + 2 bords latéraux = largeur capsule) contre les colliders SOLIDES
// (`exclude_sensors` → on ignore les autres ennemis), clamp conservateur (jamais
// de pénétration) + slide le long de la surface pour contourner.

// 2026-08-13 — `BOT_BODY_RADIUS_M = 0.4` a été SUPPRIMÉE.
//
// Elle se disait « valeur moyenne, marge conservatrice ». Elle n'était
// conservatrice pour personne : le tank (0,55) et le boss (1,40) pénétraient la
// géométrie, le runner (0,32) et le sniper (0,30) s'arrêtaient 10 cm trop tôt ET
// rataient leur glissement, parce que le test de dégagement latéral demandait une
// largeur qu'ils n'avaient pas. Deux symptômes opposés, une seule cause : une
// grandeur écrite deux fois — une fois en génome, une fois en dur.
//
// Le rayon vient désormais d'`ArenaBot::body_radius_m`, lu du même génome que le
// collider lui-même.
/// Marge anti-pénétration : le bord du bot s'arrête à cette distance du mur.
const COLLIDE_SKIN_M: f32 = 0.08;

/// Décide de l'altitude d'arrivée d'un pas — **PUR**, donc testable.
///
/// Story-685. Avant, `bot_tactical_movement` travaillait en XZ pur et il
/// n'existait ni gravité, ni suivi de sol, ni hauteur de marche dans tout le
/// crate : un bot restait au Y où il était né. Sur du relief il flottait ou
/// s'enterrait ; dans un escalier il le traversait. C'est cette pièce absente
/// qui bloquait relief, escaliers et étages d'un seul coup.
///
/// Trois issues, et le refus en fait partie :
/// - le sol monte de moins que `max_up` → on monte (marche gravie) ;
/// - il descend de moins que `max_down` → on descend (marche dévalée) ;
/// - sinon → `None`, le pas est REFUSÉ. Une paroi ne s'escalade pas (le bot n'a
///   ni saut ni grimpe) et un vide ne se franchit pas : sans ce refus, un bot
///   sortirait de l'arène par le premier rebord.
///
/// `ground_y` à `None` = aucun sol trouvé sous le pas → refus, même raison.
/// `foot_offset` = distance des pieds au centre du `Transform`. Le raisonnement
/// se fait sur les PIEDS ; le résultat est réexprimé en centre. Confondre les
/// deux enterre chaque bot de la hauteur de sa capsule.
pub fn resolve_step_altitude(
    current_y: f32,
    ground_y: Option<f32>,
    foot_offset: f32,
    max_up: f32,
    max_down: f32,
) -> Option<f32> {
    match resolve_step(current_y, ground_y, foot_offset, max_up, max_down) {
        StepVerdict::Accepte { y, .. } => Some(y),
        StepVerdict::Refuse(_) => None,
    }
}

/// Pourquoi un pas a été refusé. **Trois causes, trois remèdes opposés.**
///
/// Elles étaient toutes écrasées en un `None` muet, donc un bot planté par une paroi
/// et un bot planté par un trou étaient rigoureusement indiscernables — y compris
/// quand la sonde elle-même était fausse (défaut du 2026-08-13 : elle ne descendait
/// que 10 cm, donc tout ressemblait à `SolAbsent`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StepRefusal {
    /// Aucun sol sous le pas dans la portée de la sonde. Trou, bord d'arène — ou
    /// sonde trop courte, ce qui est un défaut d'outil, pas de décor.
    SolAbsent,
    /// Le sol monte plus que `max_step_up_m` : c'est une paroi. Le bot n'a ni saut
    /// ni grimpe, il doit CONTOURNER — donc si ça persiste, c'est le maillage qui
    /// aurait dû déclarer cet obstacle et ne l'a pas fait.
    ParoiTropHaute { montee_m: f32 },
    /// Le sol descend plus que `max_step_down_m` : un vide. Refuser est correct,
    /// sinon un bot sortirait de l'arène par le premier rebord.
    VideTropProfond { descente_m: f32 },
}

/// Le verdict complet d'un pas — l'altitude ET, en cas de refus, sa cause.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StepVerdict {
    Accepte {
        /// Altitude d'arrivée du CENTRE.
        y: f32,
        /// Dénivelé franchi (positif = montée), en mètres.
        denivele_m: f32,
    },
    Refuse(StepRefusal),
}

/// Même décision que [`resolve_step_altitude`], mais qui **dit pourquoi**.
///
/// C'est la fonction de vérité ; `resolve_step_altitude` n'en est que la projection
/// historique. Séparer les deux garde les tests existants valides tout en rendant la
/// cause observable — sans elle, `forgia2_bot_traces.json` ne pourrait afficher qu'un
/// « refusé » qui ne guide vers rien.
#[must_use]
pub fn resolve_step(
    current_y: f32,
    ground_y: Option<f32>,
    foot_offset: f32,
    max_up: f32,
    max_down: f32,
) -> StepVerdict {
    let Some(g) = ground_y else {
        return StepVerdict::Refuse(StepRefusal::SolAbsent);
    };
    let feet = current_y - foot_offset;
    let delta = g - feet;
    if delta > max_up {
        return StepVerdict::Refuse(StepRefusal::ParoiTropHaute { montee_m: delta });
    }
    if delta < -max_down {
        return StepVerdict::Refuse(StepRefusal::VideTropProfond {
            descente_m: -delta,
        });
    }
    StepVerdict::Accepte {
        y: g + foot_offset,
        denivele_m: delta,
    }
}

/// Valide un déplacement XZ contre les murs solides. Retourne le déplacement
/// effectif (clampé à l'impact + slide tangentiel si le couloir est dégagé).
fn collide_and_slide(
    origin: Vec3,
    step: Vec3,
    body_radius_m: f32,
    foot_offset_m: f32,
    max_step_up_m: f32,
    rapier: &RapierContext,
    self_entity: Entity,
) -> Vec3 {
    let flat = Vec3::new(step.x, 0.0, step.z);
    let len = flat.length();
    if len < 1.0e-4 {
        return step;
    }
    let dir = flat / len;
    let pred = |e: Entity| e != self_entity;
    let filter = QueryFilter::default().exclude_sensors().predicate(&pred);
    // La sonde de mur se pose JUSTE AU-DESSUS du ressaut franchissable.
    //
    // # Ce que cette hauteur décide, et pourquoi c'est le bon seuil (2026-08-13)
    //
    // Un solide plus bas que `max_step_up_m` n'est pas un mur : c'est une MARCHE.
    // Le bot doit monter dessus, pas s'arrêter devant. Un solide plus haut est un
    // mur : il doit le contourner. Poser la sonde à `pieds + max_step_up` fait
    // exactement ce tri, **sans interroger la moindre entité** — ce qui est au-
    // dessus est touché, ce qui est en dessous passe sous le rayon.
    //
    // C'est aussi la SEULE façon d'aligner les trois consommateurs du même seuil :
    //
    // | qui | ce qu'il en fait |
    // |---|---|
    // | maillage (`blocks_agent`) | `h > step` ⇒ obstacle à contourner |
    // | sonde de sol (`resolve_step`) | montée ≤ `step` ⇒ pas accepté |
    // | **cette sonde de mur** | plus haut que `step` ⇒ mur |
    //
    // Avant : `origin + Y * 0.5`, où `origin` est le CENTRE. Soit 1,60 m au-dessus
    // des pieds pour un bot d'arène — la sonde ne voyait donc aucun prop sous
    // 1,60 m, alors que le maillage en déclare obstacle dès 0,45 m. Un prop entre
    // les deux était contourné par le chemin, invisible à la collision, et refusé
    // par la sonde de sol : le bot gelait avec `ParoiTropHaute` sans qu'aucune des
    // trois pièces ne soit d'accord avec les autres.
    //
    // Pente maximale que ce choix laisse passer : `atan(max_step_up / max_toi)`,
    // soit ~30° pour nos valeurs. Les rampes du projet sont à 14° (dérivées de
    // `MAX_SLOPE_DEG` puis arrondies au module) — marge confortable.
    let pieds_y = origin.y - foot_offset_m;
    let probe = Vec3::new(origin.x, pieds_y + max_step_up_m + COLLIDE_SKIN_M, origin.z);
    let lateral = Vec3::new(-dir.z, 0.0, dir.x) * body_radius_m;
    let max_toi = len + body_radius_m + COLLIDE_SKIN_M;
    // 3 rayons parallèles : centre + bords gauche/droit (couvre la largeur capsule
    // → ne franchit pas un coin de mur qu'un rayon central seul manquerait).
    let mut best: Option<(f32, Vec3)> = None; // (time_of_impact, normale XZ)
    for off in [Vec3::ZERO, lateral, -lateral] {
        if let Some((_, hit)) =
            rapier.cast_ray_and_get_normal(probe + off, dir, max_toi, true, filter)
        {
            if best.is_none_or(|(bt, _)| hit.time_of_impact < bt) {
                let n = Vec3::new(hit.normal.x, 0.0, hit.normal.z).normalize_or_zero();
                best = Some((hit.time_of_impact, n));
            }
        }
    }
    let Some((toi, normal)) = best else {
        return step; // rien devant → déplacement complet
    };
    // Conservateur : on retire rayon + skin → le bord ne pénètre jamais le mur.
    let allowed = (toi - body_radius_m - COLLIDE_SKIN_M).clamp(0.0, len);
    let mut moved = dir * allowed;
    // Slide : projette le reste sur la tangente du mur, si le couloir est dégagé.
    if normal != Vec3::ZERO {
        let remaining = len - allowed;
        let slide_dir = (dir - normal * dir.dot(normal)).normalize_or_zero();
        if slide_dir != Vec3::ZERO {
            let slide_clear = rapier
                .cast_ray(
                    probe,
                    slide_dir,
                    remaining + body_radius_m + COLLIDE_SKIN_M,
                    true,
                    filter,
                )
                .is_none_or(|(_, t)| t > body_radius_m + COLLIDE_SKIN_M);
            if slide_clear {
                moved += slide_dir * remaining;
            }
        }
    }
    Vec3::new(moved.x, 0.0, moved.z)
}

/// Override state machine V1 — Chase intelligent : forward + strafe + obstacle avoidance.
/// Run APRÈS `bot_state_machine` original pour appliquer le tactical layer.
#[allow(clippy::too_many_arguments)]
pub fn bot_tactical_movement(
    mut bots: Query<
        (
            Entity,
            &mut ArenaBot,
            &mut Transform,
            Option<&crate::navpath::BotPath>,
            Option<&mut BotTrace>,
        ),
        Without<BotTarget>,
    >,
    targets: Query<&Transform, With<BotTarget>>,
    rapier: ReadRapierContext,
    tuning: Res<TacticalTuning>,
    time: Res<Time>,
    mut sensor: ResMut<BotAiSensor>,
) {
    let Some(target_tf) = targets.iter().next() else {
        return;
    };
    let target_pos = target_tf.translation;
    let Ok(ctx) = rapier.single() else { return };
    let dt = time.delta_secs();

    for (bot_entity, mut bot, mut xf, nav_path, mut trace) in &mut bots {
        if bot.state == BotState::Dead {
            continue;
        }
        // Alerted → force Chase si hors detect_range mais dans alert (audio AI).
        // Story-464 — gate sur has_los : sans vue récente ni alert, le bot ne
        // doit pas Chase aveuglément à travers les murs. La state machine
        // downgrade déjà Chase → Idle, mais on protège aussi ici en cas de
        // course (state machine run avant nous mais override possible).
        let has_recent_sight = bot.has_los || bot.los_lost_grace_left > 0.0;
        let to_target = target_pos - xf.translation;
        let dist = to_target.length();
        let want_chase = (matches!(bot.state, BotState::Chase) && has_recent_sight)
            || (bot.alerted && dist > bot.stop_distance);
        if !want_chase || bot.speed < 0.01 || dist < 0.01 {
            continue;
        }
        // Story-700 inc.3 — c'est LA ligne qui portait « fonce vers la cible », et la
        // seule. Le chemin du navmesh la remplace ; strafe, évitement local, glissement
        // contre les murs et suivi de sol restent inchangés en dessous.
        //
        // Le repli en ligne droite est explicite et non négociable : hors arène, pendant
        // le chargement, ou si la cible est hors du maillage, le bot se comporte
        // EXACTEMENT comme avant cet incrément. On n'échange pas un défaut connu contre
        // un bot immobile.
        let straight = (to_target / dist).with_y(0.0).normalize_or_zero();
        let fwd_dir = nav_path
            .and_then(crate::navpath::BotPath::current)
            .map(|w| {
                Vec3::new(w.x - xf.translation.x, 0.0, w.y - xf.translation.z)
                    .normalize_or_zero()
            })
            .filter(|d| d.length_squared() > 0.0)
            .unwrap_or(straight);

        // Story-700 inc.3b — TRAVERSER n'est pas ENGAGER.
        //
        // Symptôme du 2026-08-13 : « les mobs se bloquent dans les portes et les
        // couloirs ». Trois forces poussaient le bot hors de son chemin, et toutes les
        // trois ont été conçues pour un bot QUI N'EN AVAIT PAS :
        //
        //   strafe            ±1,80 m  — dans une porte de 4 m dilatée à 3 m de passage
        //                                utile, il garantit de toucher le montant
        //   évitement local    2,50 m  — il voit les DEUX montants et fuit le passage
        //                                que le chemin lui désigne
        //   rayon d'arrivée    1,00 m  — validait la porte de loin, d'où le virage coupé
        //                                en plein dans le cadre (corrigé à 0,35 m)
        //
        // Tant qu'il reste un tronçon APRÈS celui-ci, le bot traverse : le chemin est
        // déjà sans collision (obstacles dilatés du rayon d'agent), donc on le suit au
        // pied de la lettre. Sur le DERNIER tronçon il engage, et strafe et évitement
        // reprennent la main — c'est là qu'ils servent, en terrain ouvert face au joueur.
        let en_traversee = nav_path.is_some_and(|p| !p.is_final_leg());
        let strafe = compute_strafe_offset(&mut bot, fwd_dir, &tuning, dt);
        let desired = if en_traversee {
            fwd_dir
        } else {
            (fwd_dir + strafe.normalize_or_zero() * 0.4).normalize_or_zero()
        };
        // Phase 3 obstacle avoidance.
        // En sortie d'obstacle on LONGE au lieu de foncer : l'évitement local a
        // déjà échoué (son repli est `fwd_dir`, c'est-à-dire droit dans le mur).
        let final_dir = if bot.unstick_left > 0.0 {
            let cote = unstick_side(bot.strafe_noise_seed);
            Vec3::new(-fwd_dir.z, 0.0, fwd_dir.x).normalize_or_zero() * cote
        } else if en_traversee {
            // On fait confiance au chemin. L'évitement local est le repli de celui qui
            // n'en a pas — le laisser corriger un trajet déjà valide, c'est exactement
            // ce qui refermait les portes.
            fwd_dir
        } else {
            pick_avoid_direction(
                xf.translation,
                desired,
                &ctx,
                bot_entity,
                tuning.local_avoid_dist_m,
            )
            .unwrap_or(fwd_dir) // fallback forward simple si tous bloqués
        };
        let step = final_dir * bot.speed * dt;

        // ── Traversée d'exception ────────────────────────────────────────────
        //
        // Demandé en jeu le 2026-08-13 : un mob bloqué depuis 2-3 s par un objet,
        // ALORS QU'IL POURSUIT, finit par le franchir. Mieux vaut un ennemi qui
        // traverse un prop qu'un ennemi planté que le joueur contemple.
        //
        // Les trois gardes sont la raison pour laquelle ça reste acceptable :
        //  - `stuck_secs` accumule la NON-PROGRESSION, pas la présence d'un
        //    obstacle : frôler un mur en avançant ne compte pas ;
        //  - `en_poursuite` : un bot au repos coincé le reste. Le voir traverser
        //    un mur pour rien serait pire que le bug qu'on corrige ;
        //  - `phase_max_secs` borne l'épisode à ~3,5 m — l'épaisseur d'un prop,
        //    pas la longueur d'un couloir.
        let (phase_left, declenche) = phase_step(
            bot.phase_left,
            bot.stuck_secs,
            bot.state == BotState::Chase,
            dt,
            &tuning,
        );
        if declenche {
            bot.stuck_secs = 0.0;
            sensor.phase_triggered_session = sensor.phase_triggered_session.saturating_add(1);
        }
        bot.phase_left = phase_left;
        let traverse = phase_left > 0.0;

        // Anti-traversée : clamp/slide le déplacement contre les murs solides
        // (le kinematic ne s'arrête pas tout seul sur du Fixed). Sauf pendant une
        // traversée d'exception, où le pas passe tel quel.
        let safe = if traverse {
            step
        } else {
            collide_and_slide(
                xf.translation,
                step,
                bot.body_radius_m,
                bot.foot_offset_m,
                tuning.max_step_up_m,
                &ctx,
                bot_entity,
            )
        };
        // Story-685 — SUIVI DE SOL. Le pas est d'abord résolu en XZ, puis on
        // cherche le sol à l'arrivée. Avant, le commentaire disait « Y stays at
        // spawn » et c'était exact : un bot restait à son altitude de naissance
        // pour toujours, donc il flottait sur le relief et traversait les
        // escaliers. C'est cette pièce qui bloquait relief, marches et étages.
        let next = Vec3::new(
            xf.translation.x + safe.x,
            xf.translation.y,
            xf.translation.z + safe.z,
        );
        // Ce que le mur a laissé passer, avant même la question du sol. Une butée
        // franche (frac ≈ 0) et un pas refusé en altitude produisent le même
        // `parcouru == 0` — les séparer est tout l'objet de la trace.
        let vise_xz = step.with_y(0.0).length();
        let mur_frac = if vise_xz > 1.0e-5 {
            safe.with_y(0.0).length() / vise_xz
        } else {
            1.0
        };
        let ground = ground_under(next, bot.foot_offset_m, &ctx, bot_entity, &tuning);
        // `None` = pas REFUSÉ : paroi trop haute, vide trop profond, ou pas de
        // sol. Le bot reste sur place plutôt que d'escalader (il n'a ni saut ni
        // grimpe) ou de sortir de l'arène par un rebord. `bot_separation` et
        // l'évitement le feront glisser au tick suivant.
        let avant = xf.translation;
        let verdict = resolve_step(
            xf.translation.y,
            ground,
            bot.foot_offset_m,
            tuning.max_step_up_m,
            tuning.max_step_down_m,
        );
        match verdict {
            StepVerdict::Accepte { y, .. } => {
                xf.translation.x = next.x;
                xf.translation.z = next.z;
                xf.translation.y = y;
            }
            // Pendant une traversée, le sol refuserait aussi le pas — le sommet du
            // prop qu'on franchit est justement trop haut. On force donc le XZ,
            // mais on GARDE l'altitude courante au lieu de l'ignorer : traverser un
            // caisson ne doit pas devenir tomber d'un rebord. Le pas suivant hors
            // traversée remettra le bot sur son sol.
            StepVerdict::Refuse(_) if traverse => {
                xf.translation.x = next.x;
                xf.translation.z = next.z;
            }
            StepVerdict::Refuse(_) => {}
        }
        // Ce qu'il a RÉELLEMENT parcouru, pas ce qu'il visait. Un pas refusé en
        // altitude vaut zéro ici — c'est justement un des deux chemins qui
        // laissaient un bot planté sans que rien ne le voie.
        let parcouru = (xf.translation - avant).with_y(0.0).length();
        if let Some(trace) = trace.as_mut() {
            trace.mur_frac = mur_frac;
            if let StepVerdict::Refuse(cause) = verdict {
                trace.dernier_refus = Some(cause);
            }
            trace.tick(xf.translation, parcouru, dt);
        }
        let etat = unstick_step(
            StuckState {
                stuck_secs: bot.stuck_secs,
                unstick_left: bot.unstick_left,
            },
            parcouru,
            bot.speed * dt,
            dt,
            &tuning,
        );
        // Front montant seulement : on compte les DÉCLENCHEMENTS, pas les
        // frames passées en sortie — sinon le nombre dirait la durée.
        if etat.is_escaping() && bot.unstick_left <= 0.0 {
            sensor.unstick_triggered_session = sensor.unstick_triggered_session.saturating_add(1);
        }
        bot.stuck_secs = etat.stuck_secs;
        bot.unstick_left = etat.unstick_left;
    }
}

// ─── Traversée d'exception ───────────────────────────────────────────────────

/// Décide de la traversée d'un obstacle — **PUR**, donc testable sans moteur.
///
/// Rend `(temps de traversée restant, déclenchement à l'instant)`. Le second sert
/// au compteur : on compte les DÉCLENCHEMENTS, pas les frames passées à traverser,
/// sinon le nombre dirait la durée.
///
/// # Les trois gardes, et pourquoi chacun
///
/// 1. **`stuck_secs >= phase_after_secs`** — il faut une non-progression soutenue.
///    `stuck_secs` mesure l'absence d'avance, pas la présence d'un obstacle :
///    frôler un mur en avançant ne l'alimente pas.
/// 2. **`en_poursuite`** — un bot au repos coincé le reste. Le voir franchir un mur
///    sans raison serait plus dommageable que le blocage qu'on corrige.
/// 3. **`phase_max_secs`** — l'épisode est borné à ~3,5 m à 3,5 m/s : l'épaisseur
///    d'un prop, pas la longueur d'un couloir.
///
/// Une traversée en cours ne se re-déclenche pas : le premier test exige
/// `phase_left <= 0`, sinon un bot toujours bloqué renouvellerait indéfiniment son
/// laissez-passer et traverserait l'arène entière.
#[must_use]
pub fn phase_step(
    phase_left: f32,
    stuck_secs: f32,
    en_poursuite: bool,
    dt: f32,
    tuning: &TacticalTuning,
) -> (f32, bool) {
    if phase_left <= 0.0 && en_poursuite && stuck_secs >= tuning.phase_after_secs {
        // Déclenché À L'INSTANT : la durée pleine, moins ce tick.
        return ((tuning.phase_max_secs - dt).max(0.0), true);
    }
    ((phase_left - dt).max(0.0), false)
}

// ─── Trace individuelle ──────────────────────────────────────────────────────

/// Fenêtre d'observation de la progression (s). Assez longue pour qu'un
/// contournement normal ne passe pas pour un piétinement (faire le tour d'un
/// bâtiment de 12 m à 3,5 m/s prend ~7 s), assez courte pour qu'un blocage se voie
/// dans la seconde qui suit.
const FENETRE_PROGRESSION_S: f32 = 3.0;

/// Ce que fait UN bot, tick après tick. Sans ça, seul l'agrégat existe — et
/// l'agrégat ne sait pas distinguer « 5 bots qui vont bien » de « 4 qui vont bien
/// et 1 planté depuis 30 s ».
///
/// # La distinction que ce composant existe pour rendre
///
/// Rapporté en jeu le 2026-08-13 : *« ceux qui avancent mais ne bougent pas »*.
/// C'est une observation précise, et elle décrit **deux pannes différentes** que
/// `parcouru` seul confond :
///
/// | `parcouru` cumulé | déplacement NET | ce que c'est |
/// |---|---|---|
/// | ~0 | ~0 | **figé** — le pas est refusé (voir `dernier_refus`) |
/// | élevé | ~0 | **il piétine** — il marche, mais oscille ou tourne en rond |
/// | élevé | élevé | il progresse, tout va bien |
///
/// Le second cas ne déclenche PAS le chien de garde actuel, qui ne regarde que la
/// distance parcourue par tick : un bot qui fait des allers-retours de 5 cm le
/// satisfait pleinement. C'est très probablement ce qui reste après le correctif
/// de la sonde de sol.
#[derive(Component, Debug, Clone)]
pub struct BotTrace {
    /// Distance réellement parcourue depuis le début de la fenêtre (m).
    pub cumul_m: f32,
    /// Position au début de la fenêtre — sert au déplacement NET.
    pub ancre: Vec3,
    /// Temps écoulé dans la fenêtre courante (s).
    pub fenetre_s: f32,
    /// Déplacement net sur la DERNIÈRE fenêtre close (m).
    pub net_m: f32,
    /// Distance parcourue sur la dernière fenêtre close (m).
    pub cumul_clos_m: f32,
    /// Dernière cause de refus de pas, s'il y en a eu une dans la fenêtre.
    pub dernier_refus: Option<StepRefusal>,
    /// Fraction du pas voulu que `collide_and_slide` a laissé passer, au dernier
    /// tick. 0 = butée franche contre un mur, 1 = rien ne gênait.
    pub mur_frac: f32,
}

impl Default for BotTrace {
    fn default() -> Self {
        Self {
            cumul_m: 0.0,
            ancre: Vec3::ZERO,
            fenetre_s: 0.0,
            net_m: -1.0, // -1 = jamais mesuré, à distinguer de « 0 mesuré »
            cumul_clos_m: -1.0,
            dernier_refus: None,
            mur_frac: 1.0,
        }
    }
}

impl BotTrace {
    /// Ferme la fenêtre si elle est échue et publie net/cumul. Pur hors de `pos`.
    pub fn tick(&mut self, pos: Vec3, parcouru_m: f32, dt: f32) {
        self.cumul_m += parcouru_m;
        self.fenetre_s += dt;
        if self.fenetre_s >= FENETRE_PROGRESSION_S {
            self.net_m = (pos - self.ancre).with_y(0.0).length();
            self.cumul_clos_m = self.cumul_m;
            self.ancre = pos;
            self.cumul_m = 0.0;
            self.fenetre_s = 0.0;
            self.dernier_refus = None;
        }
    }

    /// **Il marche sans avancer** : il a parcouru de la distance, mais n'a
    /// pratiquement pas bougé de place. `None` tant qu'aucune fenêtre n'est close —
    /// « pas encore mesuré » n'est pas « tout va bien » (`map-design-patterns.md` §13).
    #[must_use]
    pub fn pietine(&self, vitesse_m_s: f32) -> Option<bool> {
        if self.cumul_clos_m < 0.0 {
            return None;
        }
        // Seuils DÉRIVÉS, pas choisis : « il a marché » = au moins la moitié de ce
        // que sa vitesse permettait sur la fenêtre ; « il n'a pas avancé » = moins
        // d'une seconde de marche de déplacement net.
        let a_marche = self.cumul_clos_m > vitesse_m_s * FENETRE_PROGRESSION_S * 0.5;
        let n_a_pas_avance = self.net_m < vitesse_m_s;
        Some(a_marche && n_a_pas_avance)
    }

    /// **Il est figé** : il n'a même pas marché. Le pas lui est refusé.
    #[must_use]
    pub fn fige(&self, vitesse_m_s: f32) -> Option<bool> {
        if self.cumul_clos_m < 0.0 {
            return None;
        }
        Some(self.cumul_clos_m < vitesse_m_s * FENETRE_PROGRESSION_S * 0.1)
    }
}

// ─── Chien de garde de désenlisement ─────────────────────────────────────────

/// L'état d'enlisement d'un bot, isolé pour être décidable sans moteur.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct StuckState {
    /// Temps cumulé en poursuite sans progresser (s).
    pub stuck_secs: f32,
    /// Temps restant à longer l'obstacle (s). > 0 ⇒ on ne fonce plus.
    pub unstick_left: f32,
}

impl StuckState {
    /// Le bot longe-t-il actuellement un obstacle ?
    #[inline]
    pub fn is_escaping(self) -> bool {
        self.unstick_left > 0.0
    }
}

/// Fait avancer l'état d'enlisement d'un pas.
///
/// ## Pourquoi ce chien de garde existe
///
/// Les bots de Forgia n'ont **pas de navmesh** : ils vont vers leur cible en
/// ligne droite. Deux chemins les laissent immobiles pour toujours —
/// `collide_and_slide` qui rend un pas nul contre un prop, et
/// `resolve_step_altitude` qui refuse le pas (paroi trop haute, vide). Dans les
/// deux cas, rien ne les rattrapait : « certains mobs se bloquent à cause des
/// décors et donc arrêtent de me suivre », rapporté en jeu le 2026-08-04.
///
/// C'est le manque explicitement nommé par `spawn-clearance.md` §5, qui
/// interdisait de prétendre que « ça n'arrive jamais » tant qu'il manquait.
///
/// ## Ce qu'il ne fait PAS
///
/// Il ne **téléporte** pas et ne repositionne pas : il change une DIRECTION. Un
/// bot désenlisé longe l'obstacle et reste soumis à la collision — corriger un
/// blocage en traversant le décor échangerait un défaut contre un pire.
///
/// Le seuil est **relatif** à la vitesse voulue, pas absolu : un bot qui rabote
/// un mur avance encore un peu tout en tournant en rond, et un seuil absolu ne
/// le verrait jamais.
pub fn unstick_step(
    mut state: StuckState,
    moved_m: f32,
    wanted_m: f32,
    dt: f32,
    tuning: &TacticalTuning,
) -> StuckState {
    if state.unstick_left > 0.0 {
        state.unstick_left = (state.unstick_left - dt).max(0.0);
        // La fenêtre de sortie se termine sur une ardoise nette : sinon le bot
        // re-déclenche immédiatement et reste en sortie perpétuelle, sans
        // jamais retenter d'aller vers sa cible.
        if state.unstick_left == 0.0 {
            state.stuck_secs = 0.0;
        }
        return state;
    }
    // Un pas voulu nul (bot à l'arrêt, cible atteinte) n'est pas un enlisement.
    if wanted_m <= f32::EPSILON {
        state.stuck_secs = 0.0;
        return state;
    }
    if moved_m < wanted_m * tuning.stuck_progress_frac {
        state.stuck_secs += dt;
        if state.stuck_secs >= tuning.stuck_after_secs {
            state.unstick_left = tuning.unstick_secs;
        }
    } else {
        state.stuck_secs = 0.0;
    }
    state
}

/// Le côté vers lequel un bot longe l'obstacle, dérivé de sa graine.
///
/// Dérivé et non tiré : deux bots coincés au même endroit doivent pouvoir
/// partir de deux côtés, sinon un paquet entier longe le même mur dans le même
/// sens et reste groupé contre lui.
#[inline]
pub fn unstick_side(seed: u32) -> f32 {
    if seed & 1 == 0 {
        1.0
    } else {
        -1.0
    }
}

/// Altitude du sol sous `pos`, ou `None` si rien n'est trouvé dans la fenêtre.
///
/// Le rayon part AU-DESSUS du bot (`ground_probe_height_m`) pour voir une marche
/// montante : parti de ses pieds, il manquerait toute élévation.
fn ground_under(
    pos: Vec3,
    foot_offset_m: f32,
    rapier: &RapierContext,
    self_entity: Entity,
    tuning: &TacticalTuning,
) -> Option<f32> {
    let pred = |e: Entity| e != self_entity;
    let filter = QueryFilter::default().exclude_sensors().predicate(&pred);
    // La sonde part au-dessus des PIEDS, pas du centre.
    //
    // # Le défaut corrigé le 2026-08-13 — « les mobs se bloquent dans les passages »
    //
    // Ce commentaire disait déjà « pas du centre », et le code passait pourtant
    // `pos.y`, qui EST le centre. Comme la longueur du rayon reste
    // `probe + max_step_down`, tout le décalage était mangé sur le BAS : le rayon
    // s'arrêtait `foot_offset` trop haut.
    //
    // | archétype | `foot_offset` | descente sondée | voulue |
    // |---|---|---|---|
    // | runner | 0,75 m | 0,45 m | 1,20 m |
    // | sniper | 0,82 m | 0,38 m | 1,20 m |
    // | tank   | 1,03 m | 0,17 m | 1,20 m |
    // | boss   | 1,80 m | **−0,60 m** | 1,20 m |
    //
    // Conséquence : un tank ne pouvait pas descendre une marche de 20 cm, et le
    // rayon du boss finissait 60 cm AU-DESSUS de ses propres pieds — il ne
    // trouvait donc jamais de sol, et chacun de ses pas était refusé.
    //
    // C'est la classe de défaut n°1 du projet, prise à l'envers :
    // `resolve_step_altitude` deux fonctions plus haut prévient explicitement que
    // « confondre les deux enterre chaque bot de la hauteur de sa capsule ». Le
    // garde était écrit ; il n'était pas appliqué ici.
    let (origin_y, max_toi) = ground_ray(pos.y, foot_offset_m, tuning);
    let from = Vec3::new(pos.x, origin_y, pos.z);
    rapier
        .cast_ray(from, Vec3::NEG_Y, max_toi, true, filter)
        .map(|(_, toi)| from.y - toi)
}

/// Origine (Y) et longueur du rayon de sol — **PUR**, donc vérifiable sans moteur.
///
/// Extrait exprès : le défaut ci-dessus vivait dans deux lignes que rien ne pouvait
/// interroger. Un test qui réénoncerait `max_step_down_m` ne prouverait rien ; celui
/// qui appelle CETTE fonction mesure le rayon réellement lancé, pour chaque archétype.
#[must_use]
pub fn ground_ray(center_y: f32, foot_offset_m: f32, tuning: &TacticalTuning) -> (f32, f32) {
    let pieds_y = center_y - foot_offset_m;
    (
        pieds_y + tuning.ground_probe_height_m,
        tuning.ground_probe_height_m + tuning.max_step_down_m,
    )
}

// ─── Separation steering (story-517) ──────────────────────────────────
//
// Empêche les bots de se traverser. Pattern AAA classique : pairwise
// push-out post-movement. O(N²) acceptable jusqu'à ~50 bots.
//
// Kinematic body ne se pousse pas naturellement via physics → on push
// directement la Transform en XZ (Y reste au spawn). Min distance = 1.0m
// (assez pour silhouettes humanoïdes, < 2× capsule_radius typique 0.55).

const SEPARATION_MIN_DIST_M: f32 = 1.0;
const SEPARATION_MAX_DIST_M: f32 = 1.2;
const SEPARATION_PUSH_STRENGTH: f32 = 0.5;

pub fn bot_separation(
    mut bots: Query<(Entity, &ArenaBot, &mut Transform), Without<BotTarget>>,
    rapier: ReadRapierContext,
    tuning: Res<TacticalTuning>,
    // Perf (audit 2026-07-01) : buffers scratch réutilisés au lieu de Vec::new()/
    // HashMap::new() par frame — 0 alloc/frame, conforme scalability.md.
    mut positions: Local<Vec<(Entity, Vec3)>>,
    mut deltas: Local<bevy::platform::collections::HashMap<Entity, Vec2>>,
) {
    let Ok(ctx) = rapier.single() else {
        return;
    };
    // Snapshot positions pour comparaison stable (sinon mutation iterative biaise).
    positions.clear();
    positions.extend(bots.iter().map(|(e, _, tf)| (e, tf.translation)));
    deltas.clear();
    for i in 0..positions.len() {
        for j in (i + 1)..positions.len() {
            let (e_a, pos_a) = positions[i];
            let (e_b, pos_b) = positions[j];
            let diff = Vec2::new(pos_b.x - pos_a.x, pos_b.z - pos_a.z);
            let dist = diff.length();
            if dist < 0.01 {
                // Co-located — push aléatoire petit pour les séparer ensuite.
                let nudge = Vec2::new(0.05, 0.05);
                *deltas.entry(e_a).or_default() -= nudge;
                *deltas.entry(e_b).or_default() += nudge;
                continue;
            }
            if dist < SEPARATION_MAX_DIST_M {
                // Force linéaire entre [min,max] : pleine force à min, zéro à max.
                let overlap = (SEPARATION_MAX_DIST_M - dist) / SEPARATION_MAX_DIST_M;
                let push = (diff / dist) * overlap * SEPARATION_PUSH_STRENGTH;
                *deltas.entry(e_a).or_default() -= push;
                *deltas.entry(e_b).or_default() += push;
                // Aussi push fort si réellement overlap.
                if dist < SEPARATION_MIN_DIST_M {
                    let extra = (diff / dist) * (SEPARATION_MIN_DIST_M - dist) * 0.5;
                    *deltas.entry(e_a).or_default() -= extra;
                    *deltas.entry(e_b).or_default() += extra;
                }
            }
        }
    }
    for (entity, bot, mut tf) in &mut bots {
        if let Some(delta) = deltas.get(&entity) {
            // Anti-traversée : la poussée de séparation respecte aussi les murs, et
            // avec le rayon RÉEL du bot poussé — sinon la séparation replacerait un
            // tank dans le mur dont le mouvement venait de l'écarter.
            let safe = collide_and_slide(
                tf.translation,
                Vec3::new(delta.x, 0.0, delta.y),
                bot.body_radius_m,
                bot.foot_offset_m,
                tuning.max_step_up_m,
                &ctx,
                entity,
            );
            tf.translation.x += safe.x;
            tf.translation.z += safe.z;
        }
    }
}

// ─── Sensor `forgia_bot_ai.json` ───────────────────────────────────────

/// Attache une [`BotTrace`] à tout bot qui n'en a pas — même raison que
/// `sys_attach_bot_path` : les bots naissent dans plusieurs crates de mode, et un bot
/// sans trace reste parfaitement fonctionnel, il est juste invisible au diagnostic.
pub fn sys_attach_bot_trace(
    mut commands: Commands,
    sans_trace: Query<(Entity, &Transform), (With<ArenaBot>, Without<BotTrace>)>,
) {
    for (e, xf) in &sans_trace {
        commands.entity(e).insert(BotTrace {
            ancre: xf.translation,
            ..Default::default()
        });
    }
}

/// `forgia2_bot_traces.json` — **ce que fait CHAQUE bot**, un par ligne.
///
/// # Pourquoi un capteur de plus
///
/// `forgia_bot_ai.json` agrège, et l'agrégat a atteint sa limite : après le correctif
/// de la sonde de sol il annonce 0 chemin refusé, 0 bot hors maillage, et **3
/// désenlisements quand même**. Il ne peut pas dire lequel, ni pourquoi. Demandé en
/// jeu le 2026-08-13 : *« regarde individuellement la trajectoire de chaque mob […]
/// ceux qui avancent mais ne bougent pas »*.
///
/// Chaque ligne porte les deux verdicts dérivés — `fige` (le pas est refusé) et
/// `pietine` (il marche mais tourne en rond) — plus la **cause nommée** du dernier
/// refus. Les trois ensemble désignent le remède ; aucun ne le fait seul.
pub fn write_bot_traces_sensor(
    time: Res<Time>,
    tuning: Res<TacticalTuning>,
    bots: Query<(Entity, &ArenaBot, &Transform, &BotTrace, Option<&crate::navpath::BotPath>)>,
    mut last_write: Local<f32>,
    // Tampon réutilisé : 0 allocation par écriture (`scalability.md`).
    mut buf: Local<String>,
) {
    let now = time.elapsed_secs();
    if now - *last_write < tuning.sensor_period_secs.max(0.1) {
        return;
    }
    *last_write = now;

    buf.clear();
    let (mut figes, mut pietinants, mut aveugles) = (0u32, 0u32, 0u32);
    for (e, bot, xf, trace, path) in &bots {
        if bot.state == BotState::Dead {
            continue;
        }
        let fige = trace.fige(bot.speed);
        let pietine = trace.pietine(bot.speed);
        match (fige, pietine) {
            (None, _) => aveugles += 1,
            (Some(true), _) => figes += 1,
            (Some(false), Some(true)) => pietinants += 1,
            _ => {}
        }
        let cause = match trace.dernier_refus {
            None => "aucun".to_string(),
            Some(StepRefusal::SolAbsent) => "sol_absent".to_string(),
            Some(StepRefusal::ParoiTropHaute { montee_m }) => {
                format!("paroi_{montee_m:.2}m")
            }
            Some(StepRefusal::VideTropProfond { descente_m }) => {
                format!("vide_{descente_m:.2}m")
            }
        };
        if !buf.is_empty() {
            buf.push(',');
        }
        // `net`/`cumul` à -1 = fenêtre jamais close. « Pas encore mesuré » n'est pas
        // « tout va bien » — `map-design-patterns.md` §13.
        buf.push_str(&format!(
            r#"{{"e":{},"pos":[{:.1},{:.1},{:.1}],"etat":"{:?}","net_m":{:.2},"cumul_m":{:.2},"fige":{},"pietine":{},"refus":"{}","mur_frac":{:.2},"wp":{},"wp_total":{},"stuck_s":{:.1}}}"#,
            e.index(),
            xf.translation.x,
            xf.translation.y,
            xf.translation.z,
            bot.state,
            trace.net_m,
            trace.cumul_clos_m,
            fige.map_or("null".into(), |b| b.to_string()),
            pietine.map_or("null".into(), |b| b.to_string()),
            cause,
            trace.mur_frac,
            path.map_or(0, |p| p.cursor),
            path.map_or(0, |p| p.waypoints.len()),
            bot.stuck_secs,
        ));
    }

    let (severity, next_step) = if aveugles > 0 && figes == 0 && pietinants == 0 {
        (
            "info",
            "AVEUGLE : aucune fenetre de progression close. Le capteur ne dit PAS que \
             tout va bien, il dit qu'il n'a rien encore mesure. Laisser tourner 3 s."
                .to_string(),
        )
    } else if figes > 0 {
        (
            "error",
            format!(
                "{figes} bot(s) FIGES : leur pas est refuse. Lire `refus` — `paroi_*` = \
                 un solide que le maillage aurait du declarer obstacle (h > step_height) \
                 et n'a pas declare ; `vide_*` = un rebord, refus correct ; `sol_absent` \
                 = trou OU sonde trop courte. Croiser avec `mur_frac` proche de 0, qui \
                 designerait un mur et non le sol."
            ),
        )
    } else if pietinants > 0 {
        (
            "warn",
            format!(
                "{pietinants} bot(s) PIETINENT : ils marchent sans progresser. Le chien \
                 de garde actuel ne les voit PAS, il ne regarde que la distance par tick. \
                 Cause probable : oscillation entre deux waypoints, ou strafe qui annule \
                 l'avance. Lire `wp`/`wp_total` — un curseur qui n'avance pas le confirme."
            ),
        )
    } else {
        ("ok", "tous les bots progressent".to_string())
    };

    let lignes: &str = &buf;
    let json = format!(
        r#"{{"id":"bot_traces","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{now:.2},"figes":{figes},"pietinants":{pietinants},"aveugles":{aveugles},"bots":[{lignes}]}}"#
    );
    let _ = forgia_core::sensor_io::enqueue("forgia2_bot_traces.json", json);
}

pub fn write_bot_ai_sensor(
    time: Res<Time>,
    tuning: Res<TacticalTuning>,
    bots: Query<&ArenaBot>,
    mut sensor: ResMut<BotAiSensor>,
) {
    let now = time.elapsed_secs();
    if now - sensor.last_write_secs < tuning.sensor_period_secs.max(0.1) {
        return;
    }
    sensor.last_write_secs = now;
    let mut alive = 0u32;
    let mut with_los = 0u32;
    let mut in_grace = 0u32;
    let mut alerted = 0u32;
    let mut chasing = 0u32;
    let mut attacking = 0u32;
    // Combien longent un obstacle MAINTENANT, et combien sont en train de
    // s'enliser sans avoir encore déclenché. Sans ces deux nombres, un bot
    // planté dans le décor redevient invisible — c'est ainsi qu'il a fallu une
    // partie pour le voir.
    let mut unsticking = 0u32;
    let mut stalling = 0u32;
    for bot in &bots {
        if bot.state == BotState::Dead {
            continue;
        }
        alive += 1;
        if bot.has_los {
            with_los += 1;
        }
        // BUG-464-03 — bots actuellement en "last sight grace" (LOS perdu mais
        // grace pas encore expirée). Permet d'observer le gate story-464.
        if !bot.has_los && bot.los_lost_grace_left > 0.0 {
            in_grace += 1;
        }
        if bot.alerted {
            alerted += 1;
        }
        if bot.unstick_left > 0.0 {
            unsticking += 1;
        } else if bot.stuck_secs > 0.0 {
            stalling += 1;
        }
        match bot.state {
            BotState::Chase => chasing += 1,
            BotState::Attack => attacking += 1,
            _ => {}
        }
    }
    sensor.bots_alive = alive;
    sensor.bots_with_los = with_los;
    sensor.bots_alerted = alerted;
    sensor.bots_chasing = chasing;
    sensor.bots_attacking = attacking;
    let json = format!(
        r#"{{"timestamp_secs":{:.2},"bots_alive":{},"bots_with_los":{},"bots_in_grace":{},"bots_alerted":{},"bots_chasing":{},"bots_attacking":{},"bots_unsticking":{},"bots_stalling":{},"unstick_triggered_session":{},"phase_triggered_session":{},"paths_ok_session":{},"paths_snapped_session":{},"paths_snapped_bot_session":{},"paths_kept_session":{},"paths_failed_session":{},"last_fail_from":[{:.1},{:.1}],"last_fail_to":[{:.1},{:.1}],"last_fail_bot_off_mesh":{},"last_fail_target_off_mesh":{},"los_checks_session":{},"alerts_triggered_session":{},"tuning":{{"los_hz":{:.1},"strafe_amp_m":{:.2},"alert_radius_m":{:.1},"los_lost_grace_secs":{:.2}}}}}"#,
        now,
        alive,
        with_los,
        in_grace,
        alerted,
        chasing,
        attacking,
        unsticking,
        stalling,
        sensor.unstick_triggered_session,
        sensor.phase_triggered_session,
        sensor.paths_ok_session,
        sensor.paths_snapped_session,
        sensor.paths_snapped_bot_session,
        sensor.paths_kept_session,
        sensor.paths_failed_session,
        sensor.last_fail_from.0,
        sensor.last_fail_from.1,
        sensor.last_fail_to.0,
        sensor.last_fail_to.1,
        sensor.last_fail_bot_off_mesh,
        sensor.last_fail_target_off_mesh,
        sensor.los_checks_session,
        sensor.alerts_triggered_session,
        tuning.los_check_hz,
        tuning.strafe_amplitude_m,
        tuning.gunshot_alert_radius_m,
        tuning.los_lost_grace_secs,
    );
    let _ = forgia_core::sensor_io::enqueue("forgia_bot_ai.json", json);
}

/// La trace individuelle — « ceux qui avancent mais ne bougent pas » (2026-08-13).
#[cfg(test)]
mod trace_tests {
    use super::*;

    const V: f32 = 3.5; // vitesse d'un bot d'arène (m/s)

    fn trace_a(pos: Vec3) -> BotTrace {
        BotTrace {
            ancre: pos,
            ..Default::default()
        }
    }

    /// Fait tourner `n` ticks de `dt`, en avancant de `pas` metres a chaque fois
    /// dans la direction donnee.
    fn simule(t: &mut BotTrace, depart: Vec3, dir: Vec3, pas: f32, n: u32, dt: f32) -> Vec3 {
        let mut p = depart;
        for _ in 0..n {
            p += dir * pas;
            t.tick(p, pas, dt);
        }
        p
    }

    #[test]
    fn tant_qu_aucune_fenetre_n_est_close_le_verdict_est_none_pas_ok() {
        // `map-design-patterns.md` §13 : zero mesure n'est pas vert, c'est aveugle.
        // Un capteur qui repondrait « tout va bien » avant d'avoir mesure quoi que
        // ce soit est exactement le capteur menteur que ce projet traque.
        let t = trace_a(Vec3::ZERO);
        assert_eq!(t.fige(V), None);
        assert_eq!(t.pietine(V), None);
    }

    #[test]
    fn un_bot_qui_progresse_n_est_ni_fige_ni_pietinant() {
        let mut t = trace_a(Vec3::ZERO);
        // 3,5 m/s pendant 3 s en ligne droite = 10,5 m parcourus, 10,5 m nets.
        simule(&mut t, Vec3::ZERO, Vec3::X, V * 0.05, 70, 0.05);
        assert_eq!(t.fige(V), Some(false));
        assert_eq!(t.pietine(V), Some(false));
    }

    #[test]
    fn un_bot_dont_le_pas_est_refuse_est_fige() {
        // Le cas que le correctif de la sonde de sol adresse : `parcouru == 0` a
        // chaque tick parce que `resolve_step` refuse.
        let mut t = trace_a(Vec3::ZERO);
        for _ in 0..70 {
            t.tick(Vec3::ZERO, 0.0, 0.05);
        }
        assert_eq!(t.fige(V), Some(true));
        // Il n'a pas marche, donc il ne « pietine » pas — les deux verdicts sont
        // exclusifs, sinon le message de remediation designerait la mauvaise cause.
        assert_eq!(t.pietine(V), Some(false));
    }

    #[test]
    fn un_bot_qui_fait_des_allers_retours_pietine_sans_etre_fige() {
        // LE cas rapporte en jeu : « ceux qui avancent mais ne bougent pas ». Le
        // chien de garde actuel ne le voit PAS — il ne regarde que la distance par
        // tick, et un aller-retour de 17 cm par tick le satisfait pleinement.
        let mut t = trace_a(Vec3::ZERO);
        let pas = V * 0.05;
        let mut p = Vec3::ZERO;
        for i in 0..70 {
            let dir = if i % 2 == 0 { Vec3::X } else { -Vec3::X };
            p += dir * pas;
            t.tick(p, pas, 0.05);
        }
        assert_eq!(
            t.fige(V),
            Some(false),
            "il a bel et bien parcouru de la distance"
        );
        assert_eq!(
            t.pietine(V),
            Some(true),
            "mais son deplacement net est nul : c'est le symptome decrit"
        );
    }

    #[test]
    fn contourner_un_batiment_ne_compte_pas_comme_pietiner() {
        // Le faux positif a eviter : un detour LEGITIME parcourt beaucoup de
        // distance pour un deplacement net modeste. Si le seuil le confondait avec
        // un blocage, le capteur crierait a chaque contournement — donc plus
        // personne ne le lirait.
        let mut t = trace_a(Vec3::ZERO);
        let pas = V * 0.05;
        let mut p = Vec3::ZERO;
        // Quart de cercle de rayon 6 m : deplacement net ~8,5 m sur 10,5 parcourus.
        for i in 0..70 {
            let a = std::f32::consts::FRAC_PI_2 * i as f32 / 70.0;
            let dir = Vec3::new(a.cos(), 0.0, a.sin());
            p += dir * pas;
            t.tick(p, pas, 0.05);
        }
        assert_eq!(
            t.pietine(V),
            Some(false),
            "un contournement normal ne doit pas lever l'alerte"
        );
    }

    #[test]
    fn les_causes_de_refus_sont_distinctes_et_chiffrees() {
        // Trois causes, trois remedes opposes — et elles etaient toutes ecrasees en
        // un `None` muet.
        let (up, down, off) = (0.45, 1.2, 1.10);
        assert_eq!(
            resolve_step(10.0, None, off, up, down),
            StepVerdict::Refuse(StepRefusal::SolAbsent)
        );
        // Sol 80 cm au-dessus des pieds (pieds a 8,90) -> paroi. Comparaison
        // APPROCHEE : `10.0 - 1.10` vaut 8,900001 en f32, donc l'egalite stricte
        // sur la montee echouerait pour une raison qui n'a rien a voir avec la
        // regle testee.
        match resolve_step(10.0, Some(9.70), off, up, down) {
            StepVerdict::Refuse(StepRefusal::ParoiTropHaute { montee_m }) => {
                assert!((montee_m - 0.80).abs() < 1.0e-4, "montee {montee_m}");
            }
            autre => panic!("attendu ParoiTropHaute, obtenu {autre:?}"),
        }
        // Sol 2 m sous les pieds -> vide.
        match resolve_step(10.0, Some(6.90), off, up, down) {
            StepVerdict::Refuse(StepRefusal::VideTropProfond { descente_m }) => {
                assert!((descente_m - 2.00).abs() < 1.0e-4, "descente {descente_m}");
            }
            autre => panic!("attendu VideTropProfond, obtenu {autre:?}"),
        }
    }

    #[test]
    fn le_verdict_detaille_et_la_forme_historique_disent_la_meme_chose() {
        // `resolve_step_altitude` est desormais une projection de `resolve_step` :
        // si les deux divergeaient, la trace decrirait un mouvement qui n'a pas eu
        // lieu. C'est la classe de defaut n°1 du projet, prevenue par un test.
        let (up, down, off) = (0.45, 1.2, 1.10);
        for sol in [None, Some(9.70), Some(6.90), Some(8.90), Some(9.20)] {
            let a = resolve_step_altitude(10.0, sol, off, up, down);
            let b = match resolve_step(10.0, sol, off, up, down) {
                StepVerdict::Accepte { y, .. } => Some(y),
                StepVerdict::Refuse(_) => None,
            };
            assert_eq!(a, b, "divergence sur sol={sol:?}");
        }
    }
}

/// La sonde de sol — le défaut « les mobs se bloquent dans les passages » (2026-08-13).
#[cfg(test)]
mod sonde_de_sol_tests {
    use super::*;

    /// Les capsules RÉELLES de `roguelite_enemies.toml`, et celle d'`arena_bots.toml`.
    /// `foot_offset = demi_hauteur + rayon + 0,05` (`EnemyStats::foot_offset_m`).
    const CAPSULES: &[(&str, f32, f32)] = &[
        ("tank", 0.55, 0.43),
        ("runner", 0.32, 0.38),
        ("sniper", 0.30, 0.47),
        ("boss", 1.4, 0.35),
        ("capsule arena_bots", 0.40, 0.65),
    ];

    fn foot_offset(rayon: f32, demi_h: f32) -> f32 {
        demi_h + rayon + 0.05
    }

    /// Rayon d'agent du maillage — `assets/genomes/navmesh.toml` [agent].radius_m.
    /// Miroir inevitable (autre crate), donc teste, comme `spawn-clearance.md` §4bis
    /// l'exige.
    const NAVMESH_AGENT_RADIUS_M: f32 = 0.30;

    // ── La traversée d'exception ────────────────────────────────────────────

    #[test]
    fn un_bot_bloque_en_poursuite_finit_par_traverser() {
        let t = TacticalTuning::default();
        let (reste, declenche) = phase_step(0.0, t.phase_after_secs, true, 0.016, &t);
        assert!(declenche, "bloque depuis {} s en poursuite", t.phase_after_secs);
        assert!(reste > 0.0);
    }

    #[test]
    fn un_bot_au_repos_coince_ne_traverse_jamais() {
        // Garde n°2. Un ennemi qui franchit un mur sans raison visible est plus
        // dommageable que le blocage qu'on corrige : le joueur ne voit pas le
        // blocage d'un bot au repos, il verrait la triche.
        let t = TacticalTuning::default();
        let (_, declenche) = phase_step(0.0, 999.0, false, 0.016, &t);
        assert!(!declenche, "hors poursuite, on reste coince");
    }

    #[test]
    fn un_bloquage_court_ne_declenche_rien() {
        // Garde n°1. Un contournement serre coute une seconde d'avance : sous le
        // seuil, on ne doit pas confondre les deux.
        let t = TacticalTuning::default();
        let (_, declenche) = phase_step(0.0, t.phase_after_secs - 0.5, true, 0.016, &t);
        assert!(!declenche);
    }

    #[test]
    fn une_traversee_en_cours_ne_se_renouvelle_pas() {
        // LE garde qui evite qu'un bot toujours bloque traverse l'arene entiere en
        // renouvelant son laissez-passer a chaque frame. Sans lui, le filet
        // deviendrait un mode de deplacement.
        let t = TacticalTuning::default();
        let (_, declenche) = phase_step(0.5, 999.0, true, 0.016, &t);
        assert!(!declenche, "deja en traversee : pas de renouvellement");
    }

    #[test]
    fn la_traversee_s_epuise_et_ne_dure_pas_plus_que_son_budget() {
        // Garde n°3, verifie dans le temps : on simule jusqu'a extinction et on
        // compare la duree reelle au budget.
        let t = TacticalTuning::default();
        let dt = 0.016;
        let (mut reste, _) = phase_step(0.0, t.phase_after_secs, true, dt, &t);
        let mut duree = dt;
        let mut gardefou = 0;
        while reste > 0.0 && gardefou < 10_000 {
            // `stuck_secs` a zero : le bot avance, il n'y a rien a renouveler.
            let (r, _) = phase_step(reste, 0.0, true, dt, &t);
            reste = r;
            duree += dt;
            gardefou += 1;
        }
        assert!(
            (duree - t.phase_max_secs).abs() < 0.05,
            "traversee de {duree:.2} s pour un budget de {:.2} s",
            t.phase_max_secs
        );
        // A 3,5 m/s, la distance franchie doit couvrir un prop (rayon max ~3 m)
        // sans permettre de remonter un batiment.
        let distance = 3.5 * duree;
        assert!(
            (3.0..=8.0).contains(&distance),
            "distance traversee {distance:.1} m hors de la bande utile"
        );
    }

    #[test]
    fn le_genome_est_la_source_de_verite_pour_la_traversee() {
        // Meme piege que `los_lost_grace_secs` le matin meme : le genome ecrase la
        // Resource a chaque chargement, donc changer le defaut Rust seul est INERTE.
        // Ce test lit le TOML et compare.
        let toml = include_str!("../../../assets/genomes/arena_bots.toml");
        for (cle, attendu) in [
            ("phase_after_secs", TacticalTuning::default().phase_after_secs),
            ("phase_max_secs", TacticalTuning::default().phase_max_secs),
        ] {
            let ligne = toml
                .lines()
                .find(|l| l.trim_start().starts_with(cle))
                .unwrap_or_else(|| panic!("{cle} absent d'arena_bots.toml"));
            let valeur: f32 = ligne
                .split('=')
                .nth(1)
                .and_then(|v| v.split('#').next())
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or_else(|| panic!("valeur illisible pour {cle} : {ligne}"));
            assert!(
                (valeur - attendu).abs() < 1.0e-6,
                "{cle} : le genome dit {valeur}, le defaut Rust {attendu} — le \
                 genome GAGNE au chargement, donc le defaut serait inerte"
            );
        }
    }

    #[test]
    fn le_maillage_ne_promet_pas_des_couloirs_ou_le_bot_ne_passe_pas() {
        // Ce test N'EST PAS un garde qui bloque : il DOCUMENTE une contrainte de
        // design, chiffree, pour qu'elle cesse d'etre implicite.
        //
        // Le maillage dilate chaque obstacle du rayon d'agent, donc il garantit des
        // couloirs de `2 x radius_m` = 60 cm. Un archetype plus large que ce rayon
        // se voit tracer des chemins qu'il ne peut PAS emprunter — il y va, il bute,
        // il pousse indefiniment. C'est le « les mobs se bloquent dans les
        // passages » rapporte en jeu le 2026-08-13.
        //
        //   archetype  rayon   couloir requis   maillage promet
        //   sniper     0,30 m      0,60 m           0,60 m   OK
        //   runner     0,32 m      0,64 m           0,60 m   limite
        //   tank       0,55 m      1,10 m           0,60 m   x1,8
        //   boss       1,40 m      2,80 m           0,60 m   x4,7
        //
        // Corriger cela demande une DECISION (batir le maillage pour le plus gros,
        // ou sortir le boss du pathfinding) — pas un reglage. Tant qu'elle n'est pas
        // prise, ce test tient le chiffre a jour et empeche qu'on l'oublie.
        let mut trop_larges: Vec<(&str, f32, f32)> = Vec::new();
        for (nom, rayon, _) in CAPSULES {
            if *rayon > NAVMESH_AGENT_RADIUS_M + 1.0e-6 {
                trop_larges.push((nom, *rayon, 2.0 * rayon));
            }
        }
        println!(
            "ARCHETYPES PLUS LARGES QUE LE MAILLAGE ({:.2} m) : {trop_larges:?} — \
             couloir promis {:.2} m",
            NAVMESH_AGENT_RADIUS_M,
            2.0 * NAVMESH_AGENT_RADIUS_M
        );
        // Le sniper au moins doit passer, sinon le maillage ne sert a personne.
        assert!(
            CAPSULES
                .iter()
                .any(|(_, r, _)| *r <= NAVMESH_AGENT_RADIUS_M + 1.0e-6),
            "AUCUN archetype ne tient dans les couloirs du maillage : le rayon \
             d'agent de navmesh.toml ne decrit plus aucun ennemi du jeu"
        );
    }

    #[test]
    fn chaque_archetype_porte_son_propre_rayon_et_pas_une_moyenne() {
        // La constante `BOT_BODY_RADIUS_M = 0.4` se disait « marge conservatrice ».
        // Ce test montre qu'elle ne l'etait pour personne : pour le tank et le boss
        // elle SOUS-estime (penetration), pour le runner et le sniper elle SUR-estime
        // (arret premature + glissement rate). Une moyenne n'est pas une marge.
        const ANCIENNE_MOYENNE: f32 = 0.40;
        let (mut sous, mut sur) = (0, 0);
        for (nom, rayon, _) in CAPSULES {
            let ecart = ANCIENNE_MOYENNE - rayon;
            if ecart < -1.0e-6 {
                sous += 1;
                println!("{nom} : SOUS-estime de {:.2} m -> penetre", -ecart);
            } else if ecart > 1.0e-6 {
                sur += 1;
                println!("{nom} : SUR-estime de {ecart:.2} m -> s'arrete trop tot");
            }
        }
        assert!(
            sous > 0 && sur > 0,
            "si une valeur unique convenait a tous, ce champ serait inutile — \
             or elle se trompe dans LES DEUX SENS ({sous} sous, {sur} sur)"
        );
    }

    #[test]
    fn la_sonde_descend_bien_max_step_down_sous_les_pieds_pour_chaque_archetype() {
        // LE test qui manquait. Le rayon partait du CENTRE alors que sa longueur
        // était calculée pour partir des PIEDS : tout le décalage était mangé sur le
        // bas. Mesuré avant correctif — tank 0,17 m au lieu de 1,20, et le boss
        // -0,60 m, c'est-à-dire un rayon qui finit AU-DESSUS de ses propres pieds,
        // donc aucun sol trouvé et chaque pas refusé.
        //
        // Aucune erreur n'était levée : un bot planté ressemble à un bot qui attend.
        let t = TacticalTuning::default();
        for (nom, rayon, demi_h) in CAPSULES {
            let off = foot_offset(*rayon, *demi_h);
            let centre_y = 10.0; // altitude quelconque : le résultat doit en être indépendant
            let (origine, longueur) = ground_ray(centre_y, off, &t);
            let pieds = centre_y - off;
            let profondeur = pieds - (origine - longueur);
            assert!(
                (profondeur - t.max_step_down_m).abs() < 1.0e-5,
                "{nom} (foot_offset {off:.2} m) : la sonde descend {profondeur:.2} m \
                 sous les pieds au lieu de {:.2} — toute marche plus profonde sera \
                 refusee et le bot restera plante",
                t.max_step_down_m
            );
        }
    }

    #[test]
    fn la_sonde_part_assez_haut_pour_voir_une_marche_montante() {
        // L'autre bout du meme rayon : partir sous `max_step_up_m` rendrait une
        // marche montante invisible, et le bot buterait dessus sans jamais la gravir.
        let t = TacticalTuning::default();
        for (nom, rayon, demi_h) in CAPSULES {
            let off = foot_offset(*rayon, *demi_h);
            let (origine, _) = ground_ray(0.0, off, &t);
            let hauteur_vue = origine - (0.0 - off);
            assert!(
                hauteur_vue > t.max_step_up_m,
                "{nom} : la sonde ne voit que {hauteur_vue:.2} m au-dessus des pieds, \
                 moins que le ressaut franchissable {:.2} m",
                t.max_step_up_m
            );
        }
    }

    #[test]
    fn la_sonde_ne_depend_pas_de_l_altitude_du_bot() {
        // Une sonde dont la portee varie avec l'altitude marcherait au sol et pas a
        // l'etage — exactement le genre de defaut intermittent qui passe les tests.
        let t = TacticalTuning::default();
        let off = foot_offset(0.40, 0.65);
        let (o1, l1) = ground_ray(0.0, off, &t);
        let (o2, l2) = ground_ray(100.0, off, &t);
        assert!((l1 - l2).abs() < 1.0e-6, "longueur dependante de l'altitude");
        assert!(
            ((o2 - o1) - 100.0).abs() < 1.0e-4,
            "l'origine doit suivre le bot exactement, pas derivement"
        );
    }
}

/// Story-700 inc.3c — la poursuite, calquée sur WoW et Minecraft (2026-08-13).
#[cfg(test)]
mod poursuite_tests {
    use super::*;

    /// Vitesses MESURÉES du projet (`map-design-intention.md` §2, métriques de
    /// `map-design-patterns.md`). Ce ne sont pas des choix, ce sont des constats.
    const SPRINT_JOUEUR_MS: f32 = 9.75;
    const VITESSE_GRUNT_MS: f32 = 9.0;

    #[test]
    fn on_ne_seme_pas_un_grunt_a_la_course() {
        // LE fait qui a décidé du modèle. Un grunt tient 92 % du sprint joueur : la
        // fuite par la distance — le modèle Minecraft — ne peut PAS marcher ici.
        // C'est pour ça que la persistance est un CHRONOMÈTRE (modèle WoW) et que la
        // laisse n'est qu'un filet.
        let ecart = SPRINT_JOUEUR_MS - VITESSE_GRUNT_MS;
        assert!(ecart > 0.0, "le joueur reste plus rapide, mais de peu");
        let secondes_pour_20m = 20.0 / ecart;
        assert!(
            secondes_pour_20m > 20.0,
            "gagner 20 m demande {secondes_pour_20m:.0} s de sprint pur — si ce test \
             casse, c'est qu'un des deux genes de vitesse a bouge et que le modele de \
             poursuite doit etre rediscute"
        );
    }

    #[test]
    fn acquerir_et_retenir_sont_deux_portees_distinctes() {
        // WoW aggro ~20 yd mais laisse bien plus longue ; Minecraft donne 40 de
        // follow_range au zombie contre 16 a la plupart. Confondre les deux, c'est
        // ce que Forgia faisait — et c'est ce qui rendait les mobs amnesiques.
        let tu = TacticalTuning::default();
        let acquisition = 25.0_f32; // ArenaBot::detect_range par defaut
        assert!(
            tu.chase_leash_m > acquisition,
            "la laisse ({}) doit depasser la portee d'acquisition ({acquisition})",
            tu.chase_leash_m
        );
    }

    #[test]
    fn la_laisse_est_atteignable_dans_la_fenetre_de_persistance() {
        // Les deux limites doivent etre du meme ordre, sinon l'une est decorative :
        // une laisse hors d'atteinte ne se declencherait jamais, un chronometre trop
        // court la rendrait inutile. A 9 m/s pendant 6 s, un grunt couvre 54 m — la
        // laisse de 50 m est donc reellement franchissable.
        let tu = TacticalTuning::default();
        let portee = VITESSE_GRUNT_MS * tu.los_lost_grace_secs;
        assert!(
            portee > tu.chase_leash_m,
            "en {} s a {VITESSE_GRUNT_MS} m/s le bot couvre {portee:.0} m : la laisse \
             de {} m doit rester atteignable, sinon elle ne sert a rien",
            tu.los_lost_grace_secs,
            tu.chase_leash_m
        );
    }

    #[test]
    fn la_persistance_reprend_le_compteur_d_evade_de_wow() {
        // 6 s = le delai apres lequel un mob WoW retail cesse la poursuite quand on
        // arrete de le combattre. Emprunte, pas choisi.
        assert!((TacticalTuning::default().los_lost_grace_secs - 6.0).abs() < f32::EPSILON);
    }
}

#[cfg(test)]
mod ground_follow_tests {
    use super::*;

    fn t() -> TacticalTuning {
        TacticalTuning::default()
    }
    /// Offset pieds→centre d'un tank : `capsule_half_height + radius + 0.05`.
    const FOOT: f32 = 0.43 + 0.42 + 0.05;

    /// **LE piège de cette story.** Le `Transform` d'un bot est le CENTRE de sa
    /// capsule, pas ses pieds. Snapper ce centre sur l'altitude du sol
    /// enterrerait chaque bot de la hauteur de son corps — un défaut qui passe
    /// toutes les compilations et se voit au premier bot en jeu.
    #[test]
    fn the_bot_stands_on_the_ground_it_does_not_sink_into_it() {
        let c = t();
        let y = resolve_step_altitude(FOOT, Some(0.0), FOOT, c.max_step_up_m, c.max_step_down_m)
            .expect("un sol plat sous les pieds doit être accepté");
        assert!(
            (y - FOOT).abs() < 1e-5,
            "le bot devrait rester à {FOOT:.2} m (pieds au sol), il est à {y:.2}"
        );
        assert!(y > 0.0, "un bot posé à l'altitude du SOL est enterré");
    }

    /// Sol plat, quelle que soit l'altitude : les pieds suivent.
    #[test]
    fn flat_ground_keeps_the_feet_on_the_floor() {
        let c = t();
        for ground in [-3.0_f32, 0.0, 12.5] {
            let y = resolve_step_altitude(
                ground + FOOT,
                Some(ground),
                FOOT,
                c.max_step_up_m,
                c.max_step_down_m,
            )
            .unwrap();
            assert!((y - (ground + FOOT)).abs() < 1e-5);
        }
    }

    /// **Une marche se gravit, une paroi non.** Le bot n'a ni saut ni grimpe :
    /// au-delà de la hauteur de marche, le pas est REFUSÉ — sinon il escaladerait
    /// un mur et se retrouverait sur le toit.
    #[test]
    fn a_step_is_climbed_but_a_wall_is_refused() {
        let c = t();
        let up = c.max_step_up_m;
        assert!(
            resolve_step_altitude(FOOT, Some(up - 0.01), FOOT, up, c.max_step_down_m).is_some(),
            "une marche sous le seuil doit se gravir"
        );
        assert_eq!(
            resolve_step_altitude(FOOT, Some(up + 0.01), FOOT, up, c.max_step_down_m),
            None,
            "au-dessus du seuil, c'est une paroi : elle se contourne"
        );
        assert_eq!(
            resolve_step_altitude(FOOT, Some(4.0), FOOT, up, c.max_step_down_m),
            None,
            "un mur de 4 m ne s'escalade pas"
        );
    }

    /// Descendre est plus facile que monter — mais borné : un vide profond est
    /// un refus, sinon le bot quitterait l'arène par le premier rebord.
    #[test]
    fn going_down_is_easier_than_going_up_but_still_bounded() {
        let c = t();
        assert!(
            c.max_step_down_m > c.max_step_up_m,
            "descendre doit être plus permissif que gravir"
        );
        let d = c.max_step_down_m;
        assert!(resolve_step_altitude(FOOT, Some(-d + 0.01), FOOT, c.max_step_up_m, d).is_some());
        assert_eq!(
            resolve_step_altitude(FOOT, Some(-d - 0.01), FOOT, c.max_step_up_m, d),
            None,
            "au-delà, c'est un vide : le bot ne saute pas dedans"
        );
    }

    /// **Pas de sol = pas de pas.** Sans ce refus, un bot avancerait dans le vide
    /// au bord d'une plateforme et sortirait de l'arène.
    #[test]
    fn no_ground_means_no_step() {
        let c = t();
        assert_eq!(
            resolve_step_altitude(FOOT, None, FOOT, c.max_step_up_m, c.max_step_down_m),
            None
        );
    }

    /// La sonde doit partir PLUS HAUT que la marche gravissable : partie trop
    /// bas, elle ne verrait jamais une élévation et le bot buterait sur une
    /// marche qu'il pouvait monter.
    #[test]
    fn the_probe_starts_above_the_tallest_climbable_step() {
        let c = t();
        assert!(
            c.ground_probe_height_m > c.max_step_up_m,
            "sonde {:.2} m ≤ marche {:.2} m — une marche montante serait invisible",
            c.ground_probe_height_m,
            c.max_step_up_m
        );
    }

    /// La hauteur de marche est SOURCÉE : `MaxStepHeight` d'Unreal, la même
    /// valeur que nos patterns de carte citent déjà. Un pas de plus de 45 cm
    /// exige une rampe, pas un saut.
    #[test]
    fn the_step_height_matches_the_documented_industry_value() {
        assert!((t().max_step_up_m - 0.45).abs() < 1e-6);
    }

    /// Un tuning dégénéré ne doit produire ni escalade ni chute infinie.
    #[test]
    fn degenerate_limits_never_let_a_bot_climb_or_fall_forever() {
        assert_eq!(resolve_step_altitude(FOOT, Some(0.5), FOOT, 0.0, 0.0), None);
        assert_eq!(
            resolve_step_altitude(FOOT, Some(-0.5), FOOT, 0.0, 0.0),
            None
        );
        // Un sol EXACTEMENT sous les pieds passe toujours, même à limites nulles.
        let y = resolve_step_altitude(FOOT, Some(0.0), FOOT, 0.0, 0.0).unwrap();
        assert!((y - FOOT).abs() < 1e-5);
    }

    /// L'offset par défaut doit rester plausible : un bot au sol, pas enterré ni
    /// en lévitation, même si son spawn oublie de le renseigner.
    #[test]
    fn the_default_foot_offset_is_plausible() {
        let d = crate::ArenaBot::default().foot_offset_m;
        assert!((0.3..=2.0).contains(&d), "offset par défaut absurde : {d}");
    }
}

#[cfg(test)]
mod unstick_tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn t() -> TacticalTuning {
        TacticalTuning::default()
    }

    /// Fait tourner `n` pas avec une progression donnée, et rend l'état final.
    fn courir(mut s: StuckState, parcouru: f32, voulu: f32, n: u32) -> StuckState {
        let tu = t();
        for _ in 0..n {
            s = unstick_step(s, parcouru, voulu, DT, &tu);
        }
        s
    }

    /// Un bot qui avance normalement ne doit JAMAIS partir en sortie
    /// d'obstacle — sinon on casse la poursuite qu'on prétend réparer.
    #[test]
    fn a_bot_that_advances_is_never_pulled_off_its_target() {
        let s = courir(StuckState::default(), 0.05, 0.05, 600);
        assert!(!s.is_escaping());
        assert_eq!(s.stuck_secs, 0.0);
    }

    /// Le cas rapporté en jeu : bloqué contre un décor, pas nul, il doit finir
    /// par longer l'obstacle.
    #[test]
    fn a_bot_pinned_against_scenery_eventually_slides_along_it() {
        let tu = t();
        let pas = (tu.stuck_after_secs / DT).ceil() as u32 + 1;
        let s = courir(StuckState::default(), 0.0, 0.05, pas);
        assert!(
            s.is_escaping(),
            "toujours planté après {:.2} s",
            tu.stuck_after_secs
        );
    }

    /// Le piège du seuil ABSOLU : un bot qui rabote un mur avance encore un
    /// peu tout en tournant en rond. Le seuil étant relatif, il est vu.
    #[test]
    fn grazing_a_wall_still_counts_as_stuck() {
        let tu = t();
        let voulu = 0.05;
        let rabote = voulu * tu.stuck_progress_frac * 0.5;
        let pas = (tu.stuck_after_secs / DT).ceil() as u32 + 1;
        assert!(courir(StuckState::default(), rabote, voulu, pas).is_escaping());
    }

    /// La sortie se TERMINE, et sur une ardoise nette : sans ça le bot
    /// re-déclenche au pas suivant et ne revient jamais vers sa cible.
    #[test]
    fn the_escape_ends_and_clears_the_slate() {
        let tu = t();
        let mut s = StuckState {
            stuck_secs: tu.stuck_after_secs,
            unstick_left: tu.unstick_secs,
        };
        let pas = (tu.unstick_secs / DT).ceil() as u32 + 1;
        s = courir(s, 0.0, 0.05, pas);
        assert!(!s.is_escaping(), "sortie perpétuelle");
        assert_eq!(s.stuck_secs, 0.0, "ardoise non remise à zéro");
    }

    /// Un bot à l'arrêt (cible atteinte, vitesse nulle) n'est pas enlisé.
    #[test]
    fn standing_still_on_purpose_is_not_being_stuck() {
        let s = courir(StuckState::default(), 0.0, 0.0, 600);
        assert!(!s.is_escaping());
    }

    /// Deux bots coincés au même mur doivent pouvoir partir de deux côtés,
    /// sinon le paquet longe le mur groupé et reste collé ensemble.
    #[test]
    fn two_bots_do_not_all_slide_the_same_way() {
        let cotes: Vec<f32> = (0..8).map(unstick_side).collect();
        assert!(cotes.contains(&1.0) && cotes.contains(&-1.0));
    }

    /// Les réglages doivent être cohérents entre eux : une fenêtre de sortie
    /// plus courte qu'un pas ne servirait à rien.
    #[test]
    fn the_escape_window_lasts_longer_than_a_single_frame() {
        let tu = t();
        assert!(tu.unstick_secs > DT * 4.0);
        assert!(tu.stuck_after_secs > 0.0);
        assert!(tu.stuck_progress_frac > 0.0 && tu.stuck_progress_frac < 1.0);
    }
}
