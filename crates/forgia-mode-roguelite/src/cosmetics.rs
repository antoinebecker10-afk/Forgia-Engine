//! cosmetics.rs — **le catalogue des cosmétiques** et la règle de possession.
//!
//! Story-678. Né comme catalogue de DÉCORS le 2026-08-06, généralisé le même
//! jour : le Marketplace vend quatre familles, et faire quatre systèmes
//! parallèles aurait donné quatre règles de possession à tenir d'accord.
//!
//! | Famille | Ce que ça change | Où c'est appliqué |
//! |---|---|---|
//! | `decor` | le fond du menu | `forgia-ui::arena_backdrop` |
//! | `color` | la couleur d'identité | `identity.rs` (panneau Forgeron) |
//! | `arm`   | couleur + style des bras (vus en permanence en FPS) | `ArmCosmetics` |
//! | `music` | le morceau qui joue au hub | `audio.rs` |
//!
//! Ce module ne rend rien et n'applique rien : il dit **ce qui existe** et **ce
//! qu'on possède**. Les consommateurs lisent l'équipé et l'appliquent.
//!
//! ## Une famille, un magasin de possession
//!
//! `has()` ne consulte pas une liste unique : chaque famille est possédée dans
//! le stock qui lui sert DÉJÀ ailleurs — les couleurs dans `unlocked_colors`
//! (que le panneau Forgeron filtre, et que le boot fait respecter), les décors
//! dans `unlocked_backdrops`, etc. Centraliser dans une cinquième liste aurait
//! fait deux vérités par famille, et le boot aurait réinitialisé une couleur
//! achetée (`sys_init_identity` remet `default` si `unlocked_colors` ne la
//! contient pas).
//!
//! ## Ce qui est dérivé, ce qui est stocké
//!
//! `source = "chapter"` se **déduit** de `chapters_cleared` : jamais stocké,
//! sinon la possession divergerait à la première renumérotation des chapitres.
//! `source = "shards"` et `"exploit"` sont **stockés** — ce sont des choix et
//! des faits, pas des calculs.

use bevy::prelude::*;
use bevy::time::Real;
use serde::Deserialize;
use std::fs;
use std::time::SystemTime;

use crate::identity::IdentitySave;

pub(crate) const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_cosmetics.toml";
const POLL_PERIOD_SEC: f32 = 1.0;

/// Décor de repli — celui du chapitre 1, possédé d'emblée. Il DOIT exister dans
/// le catalogue : un menu sans fond ne se répare pas tout seul.
pub const FALLBACK_BACKDROP: &str = "crypte_enclume";

/// Les quatre familles du Marketplace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CosmeticKind {
    Decor,
    Color,
    Arm,
    Music,
}

impl CosmeticKind {
    /// Ordre des onglets du Marketplace — du plus visible au moins visible.
    pub const ALL: [CosmeticKind; 4] = [
        CosmeticKind::Decor,
        CosmeticKind::Color,
        CosmeticKind::Arm,
        CosmeticKind::Music,
    ];

    pub fn tab_label(self) -> &'static str {
        match self {
            CosmeticKind::Decor => "🖼  Décors",
            CosmeticKind::Color => "🎨  Couleurs",
            CosmeticKind::Arm => "🧤  Bras",
            CosmeticKind::Music => "🎵  Musique",
        }
    }

    /// Ce que la famille change, dit au joueur.
    pub fn tab_help(self) -> &'static str {
        match self {
            CosmeticKind::Decor => "Le lieu qu'on voit derrière ton menu.",
            CosmeticKind::Color => "La couleur de ton forgeron.",
            CosmeticKind::Arm => "Tes bras — ceux que tu vois à chaque tir.",
            CosmeticKind::Music => "Le morceau qui joue quand tu prépares ta run.",
        }
    }

    fn from_key(s: &str) -> Option<Self> {
        match s {
            "decor" => Some(Self::Decor),
            "color" => Some(Self::Color),
            "arm" => Some(Self::Arm),
            "music" => Some(Self::Music),
            _ => None,
        }
    }
}

/// Comment un cosmétique s'obtient.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CosmeticSource {
    /// Possédé dès la première partie.
    Start,
    /// Gagné en battant ce chapitre — dérivé, jamais stocké.
    Chapter(u32),
    /// Acheté à ce prix en Éclats — stocké.
    Shards(u32),
    /// Accordé par un haut fait — stocké.
    ///
    /// Le système de hauts faits n'existe pas encore ; aucune entrée du génome
    /// n'utilise cette source (livrer un cosmétique inobtenable serait du
    /// contenu mort). Le chemin est là, testé, prêt à recevoir.
    Exploit,
}

impl CosmeticSource {
    pub fn price(self) -> Option<u32> {
        match self {
            Self::Shards(p) => Some(p),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct CosmeticToml {
    id: String,
    kind: String,
    label: String,
    source: String,
    chapter: u32,
    price: u32,
    // ── Charge utile, à plat : chaque famille lit les champs qui la concernent.
    // Un enum étiqueté par famille rendrait le TOML plus lourd à écrire pour la
    // seule vertu d'être plus strict — la validation au parse suffit.
    palette: String,
    ambiance: String,
    color: String,
    arm_style: String,
    music: String,
}

impl Default for CosmeticToml {
    fn default() -> Self {
        Self {
            id: String::new(),
            kind: String::new(),
            label: String::new(),
            source: "start".into(),
            chapter: 0,
            price: 0,
            palette: String::new(),
            ambiance: String::new(),
            color: String::new(),
            arm_style: String::new(),
            music: String::new(),
        }
    }
}

/// Barème de gain des Éclats — la monnaie COSMÉTIQUE.
///
/// Séparée des Âmes par décision du 2026-08-06 : un cosmétique acheté en Âmes
/// est un rang d'Enclume non acheté, et la courbe de puissance est justement en
/// cours de recalibrage. Deux monnaies, deux lectures qui ne se brouillent pas.
///
/// ⚠ Ces valeurs sont un PREMIER JET. Elles n'ont pas été jouées : à confronter
/// au playtest avant d'être considérées comme équilibrées.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ShardRates {
    /// Éclats par round atteint. Récompense la profondeur, pas le temps passé.
    pub per_round: u32,
    /// Prime pour un chapitre bouclé.
    pub per_chapter_cleared: u32,
}

impl Default for ShardRates {
    fn default() -> Self {
        Self {
            per_round: 1,
            per_chapter_cleared: 10,
        }
    }
}

impl ShardRates {
    /// Éclats gagnés par une run. PUR — testable.
    pub fn earned(self, rounds_reached: u32, victory: bool) -> u32 {
        let base = self.per_round.saturating_mul(rounds_reached);
        if victory {
            base.saturating_add(self.per_chapter_cleared)
        } else {
            base
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct CosmeticsToml {
    cosmetics: Vec<CosmeticToml>,
    shards: ShardRates,
}

/// Un cosmétique du catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cosmetic {
    pub id: String,
    pub kind: CosmeticKind,
    pub label: String,
    pub source: CosmeticSource,
    /// `decor` — props (clé de `DecorPalettesConfig`).
    pub palette: String,
    /// `decor` — lumière (clé de `AmbiancesConfig`). Vide = ambiance par défaut.
    pub ambiance: String,
    /// `color` et `arm` — id de couleur dans `IdentityConfig.colors`.
    pub color: String,
    /// `arm` — `peau` / `gantelet` / `cyber`.
    pub arm_style: String,
    /// `music` — clé de morceau (`hub`, `chapter_01`…).
    pub music: String,
}

/// Le catalogue (Resource, hot-reload).
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct CosmeticsConfig {
    pub items: Vec<Cosmetic>,
    pub shards: ShardRates,
}

impl Default for CosmeticsConfig {
    /// Miroir minimal : le décor de départ, pour qu'un génome absent laisse
    /// quand même un menu habillé.
    fn default() -> Self {
        Self {
            items: vec![Cosmetic {
                id: FALLBACK_BACKDROP.into(),
                kind: CosmeticKind::Decor,
                label: "La Crypte de l'Enclume".into(),
                source: CosmeticSource::Start,
                palette: "inferno".into(),
                ambiance: "forge_ardente".into(),
                color: String::new(),
                arm_style: String::new(),
                music: String::new(),
            }],
            shards: ShardRates::default(),
        }
    }
}

impl CosmeticsConfig {
    /// PUR — testable. Un génome illisible retombe sur le miroir, et le DIT.
    pub fn parse_toml(content: &str) -> Self {
        let parsed: CosmeticsToml = match toml::from_str(content) {
            Ok(v) => v,
            Err(e) => {
                warn!("[cosmetics] génome illisible ({e}) — décor de départ seul");
                return Self::default();
            }
        };
        let mut out = Vec::new();
        for c in parsed.cosmetics {
            let Some(kind) = CosmeticKind::from_key(&c.kind) else {
                warn!("[cosmetics] « {} » : famille « {} » inconnue", c.id, c.kind);
                continue;
            };
            if c.id.is_empty() {
                warn!("[cosmetics] entrée sans id — ignorée");
                continue;
            }
            // Une entrée qui ne porte pas de quoi être APPLIQUÉE est un article
            // qui ne fait rien une fois acheté. On la refuse au parse.
            let utile = match kind {
                CosmeticKind::Decor => !c.palette.is_empty(),
                CosmeticKind::Color => !c.color.is_empty(),
                CosmeticKind::Arm => !c.arm_style.is_empty() || !c.color.is_empty(),
                CosmeticKind::Music => !c.music.is_empty(),
            };
            if !utile {
                warn!(
                    "[cosmetics] « {}] » ({:?}) ne porte rien d'applicable — ignorée",
                    c.id, kind
                );
                continue;
            }
            let source = match c.source.as_str() {
                "start" => CosmeticSource::Start,
                "chapter" if c.chapter > 0 => CosmeticSource::Chapter(c.chapter),
                "shards" if c.price > 0 => CosmeticSource::Shards(c.price),
                "exploit" => CosmeticSource::Exploit,
                other => {
                    // `chapter` sans numéro ou `shards` sans prix donnerait un
                    // article gratuit par accident. Inobtenable plutôt que cadeau.
                    warn!(
                        "[cosmetics] « {}] » : source « {other} » incomplète — traitée en haut fait",
                        c.id
                    );
                    CosmeticSource::Exploit
                }
            };
            out.push(Cosmetic {
                id: c.id,
                kind,
                label: c.label,
                source,
                palette: c.palette,
                ambiance: c.ambiance,
                color: c.color,
                arm_style: c.arm_style,
                music: c.music,
            });
        }
        if out.is_empty() {
            warn!("[cosmetics] catalogue vide — décor de départ seul");
            return Self::default();
        }
        Self {
            items: out,
            shards: parsed.shards,
        }
    }

    /// Chargement direct depuis le disque — appelé à la CONSTRUCTION du plugin.
    ///
    /// Pas un système de Startup : `sys_init_identity` a besoin du catalogue
    /// pour savoir quelles couleurs restent gratuites, et faire dépendre deux
    /// systèmes de Startup l'un de l'autre (avec le point de synchronisation que
    /// ça implique pour un `insert_resource`) coûte plus cher que de simplement
    /// lire le fichier tout de suite.
    pub fn load_now() -> Self {
        Self::load_or_default()
    }

    fn load_or_default() -> Self {
        match fs::read_to_string(GENOME_PATH) {
            Ok(c) => Self::parse_toml(&c),
            Err(e) => {
                warn!("[cosmetics] {GENOME_PATH} illisible ({e}) — décor de départ seul");
                Self::default()
            }
        }
    }

    pub fn get(&self, id: &str) -> Option<&Cosmetic> {
        self.items.iter().find(|c| c.id == id)
    }

    /// Cette couleur d'identité est-elle **gouvernée par le Marketplace** ?
    ///
    /// `sys_init_identity` débloque d'office toutes les couleurs déclarées
    /// (« MVP : toutes les couleurs sont gratuites »). Tant que c'était vrai
    /// pour toutes, il n'y avait pas de conflit. Depuis que le Marketplace en
    /// vend, ce cadeau au boot rendrait l'onglet MENTEUR : il afficherait un
    /// prix pour ce que le jeu donne.
    ///
    /// La règle : une couleur **listée au catalogue avec une source autre que
    /// `start`** cesse d'être offerte — c'est le catalogue qui décide. Une
    /// couleur absente du catalogue reste gratuite, donc rien de ce qui existait
    /// ne disparaît par omission.
    pub fn color_is_governed(&self, color_id: &str) -> bool {
        self.items.iter().any(|c| {
            c.kind == CosmeticKind::Color
                && c.color == color_id
                && c.source != CosmeticSource::Start
        })
    }

    /// Les articles d'une famille, dans l'ordre du génome, avec possession.
    pub fn of_kind<'a>(
        &'a self,
        kind: CosmeticKind,
        owned: &OwnedCosmetics,
    ) -> Vec<(&'a Cosmetic, bool)> {
        self.items
            .iter()
            .filter(|c| c.kind == kind)
            .map(|c| (c, owned.has(c)))
            .collect()
    }

    /// Le DÉCOR à rendre : celui demandé s'il existe ET s'il est possédé, sinon
    /// le repli.
    ///
    /// Le contrôle de possession est ici et pas seulement dans l'UI : une
    /// sauvegarde éditée à la main ne doit pas afficher un décor non gagné.
    pub fn resolve_decor<'a>(&'a self, wanted: &str, owned: &OwnedCosmetics) -> Option<&'a Cosmetic> {
        self.get(wanted)
            .filter(|c| c.kind == CosmeticKind::Decor && owned.has(c))
            .or_else(|| self.get(FALLBACK_BACKDROP))
            .or_else(|| self.items.iter().find(|c| c.kind == CosmeticKind::Decor))
    }
}

/// De quoi juger la possession : la progression (dérivée) et les stocks de la
/// sauvegarde d'identité (stockés).
pub struct OwnedCosmetics<'a> {
    pub chapters_cleared: u32,
    pub identity: &'a IdentitySave,
}

impl OwnedCosmetics<'_> {
    /// PUR — la règle de possession, en un endroit.
    pub fn has(&self, c: &Cosmetic) -> bool {
        match c.source {
            CosmeticSource::Start => true,
            // `>=` : arriver au chapitre 5 donne aussi les articles 2 à 4.
            CosmeticSource::Chapter(n) => self.chapters_cleared >= n,
            CosmeticSource::Shards(_) | CosmeticSource::Exploit => {
                // 🚨 La clé comparée DOIT être celle que `grant_and_equip` écrit :
                // l'id de COULEUR pour la famille Color, l'id d'article sinon.
                // Comparer `c.id` (« color_azur ») à un stock qui contient
                // « azur » rendait toute couleur achetée invisible à jamais —
                // affichée « Débloquer » après paiement, donc PAYABLE DEUX FOIS.
                // Vu en jeu le 2026-08-06 ; le test unitaire vérifiait le stock
                // après achat mais jamais `has()` après achat.
                let key = match c.kind {
                    CosmeticKind::Color => c.color.as_str(),
                    _ => c.id.as_str(),
                };
                store_of(self.identity, c.kind).iter().any(|s| s == key)
            }
        }
    }
}

/// Le stock de possession d'une famille — celui qui sert DÉJÀ à cette famille
/// ailleurs dans le jeu (cf. l'en-tête du module).
///
/// Pour les couleurs, la clé stockée est l'**id de couleur** (`c.color`), pas
/// l'id d'article : c'est ce que `unlocked_colors` contient déjà et ce que le
/// panneau Forgeron filtre.
fn store_of(identity: &IdentitySave, kind: CosmeticKind) -> &[String] {
    match kind {
        CosmeticKind::Decor => &identity.unlocked_backdrops,
        CosmeticKind::Color => &identity.unlocked_colors,
        CosmeticKind::Arm => &identity.unlocked_arms,
        CosmeticKind::Music => &identity.unlocked_music,
    }
}

/// Enregistre la possession d'un article dans le stock de SA famille, et
/// l'équipe. Rend `false` s'il était déjà possédé — l'appelant sait alors qu'il
/// n'a rien débloqué et ne doit pas débiter.
///
/// 🚨 **Écrit sur le disque.** Les tests doivent appeler
/// [`grant_and_equip_in_memory`] : `save_dir()` pointe sur `%APPDATA%\Forgia`,
/// donc un test qui passe par ici **écrase la sauvegarde réelle du joueur**.
/// C'est arrivé le 2026-08-06 — un test a écrit un `IdentitySave::default()`
/// par-dessus la partie du joueur, lui faisant perdre deux couleurs gagnées et
/// fausser un achat de 40 Éclats (l'article était devenu « déjà possédé »).
pub fn grant_and_equip(identity: &mut IdentitySave, c: &Cosmetic) -> bool {
    if !grant_and_equip_in_memory(identity, c) {
        return false;
    }
    identity.persist();
    true
}

/// PUR — la même chose, SANS écriture disque. C'est cette version que testent
/// les tests, et la seule qu'ils aient le droit d'appeler.
pub fn grant_and_equip_in_memory(identity: &mut IdentitySave, c: &Cosmetic) -> bool {
    // La clé de possession des couleurs est l'id de COULEUR (cf. `store_of`).
    let key = match c.kind {
        CosmeticKind::Color => c.color.clone(),
        _ => c.id.clone(),
    };
    let store = match c.kind {
        CosmeticKind::Decor => &mut identity.unlocked_backdrops,
        CosmeticKind::Color => &mut identity.unlocked_colors,
        CosmeticKind::Arm => &mut identity.unlocked_arms,
        CosmeticKind::Music => &mut identity.unlocked_music,
    };
    if store.iter().any(|s| s == &key) {
        return false;
    }
    store.push(key);
    equip_in_memory(identity, c);
    true
}

/// Équipe un article déjà possédé.
///
/// 🚨 **Écrit sur le disque** — cf. l'avertissement de [`grant_and_equip`].
/// Les tests appellent [`equip_in_memory`].
pub fn equip(identity: &mut IdentitySave, c: &Cosmetic) {
    equip_in_memory(identity, c);
    identity.persist();
}

/// PUR — pose l'équipement en mémoire, sans toucher au disque.
pub fn equip_in_memory(identity: &mut IdentitySave, c: &Cosmetic) {
    match c.kind {
        CosmeticKind::Decor => identity.equipped_backdrop = c.id.clone(),
        CosmeticKind::Color => {
            identity.equipped_color = c.color.clone();
            // 🚨 Porter une couleur l'inscrit AUSSI dans `unlocked_colors`.
            //
            // Une couleur gagnée par CHAPITRE est possédée par dérivation : le
            // catalogue le sait, le stock ne le sait pas. Or deux consommateurs
            // plus anciens ne lisent que le stock — le panneau Forgeron (qui
            // filtre dessus) et surtout le garde de boot :
            //
            //     if !unlocked_colors.contains(&equipped_color) { equipped_color = "default" }
            //
            // Sans cette ligne, équiper « pourpre » (chapitre 4) tenait jusqu'à
            // la fermeture du jeu, puis retombait sur « default » au lancement
            // suivant, sans le moindre message. Constaté sur la sauvegarde
            // du joueur le 2026-08-06.
            if !identity.unlocked_colors.contains(&c.color) {
                identity.unlocked_colors.push(c.color.clone());
            }
        }
        CosmeticKind::Arm => {
            if !c.arm_style.is_empty() {
                identity.arm_style = c.arm_style.clone();
            }
            if !c.color.is_empty() {
                identity.arm_color = c.color.clone();
            }
        }
        CosmeticKind::Music => identity.hub_music = c.music.clone(),
    }
}

/// Cet article est-il celui actuellement porté ?
pub fn is_equipped(identity: &IdentitySave, c: &Cosmetic) -> bool {
    match c.kind {
        CosmeticKind::Decor => identity.equipped_backdrop == c.id,
        CosmeticKind::Color => identity.equipped_color == c.color,
        CosmeticKind::Arm => {
            (c.arm_style.is_empty() || identity.arm_style == c.arm_style)
                && (c.color.is_empty() || identity.arm_color == c.color)
        }
        CosmeticKind::Music => identity.hub_music == c.music,
    }
}

/// Surveillance mtime du génome (hot-reload).
#[derive(Resource, Default)]
pub struct CosmeticsWatch {
    last_mtime: Option<SystemTime>,
    pub reload_count: u32,
}

/// Journalise le catalogue une fois au boot — la ressource, elle, est déjà
/// posée par le plugin (cf. [`CosmeticsConfig::load_now`]).
pub fn sys_log_cosmetics(cfg: Res<CosmeticsConfig>) {
    info!(
        "[cosmetics] {} articles ({} décors, {} couleurs, {} bras, {} musiques)",
        cfg.items.len(),
        cfg.items.iter().filter(|c| c.kind == CosmeticKind::Decor).count(),
        cfg.items.iter().filter(|c| c.kind == CosmeticKind::Color).count(),
        cfg.items.iter().filter(|c| c.kind == CosmeticKind::Arm).count(),
        cfg.items.iter().filter(|c| c.kind == CosmeticKind::Music).count(),
    );
}

/// Poll mtime 1 Hz — éditer le catalogue prend effet sans redémarrer.
pub fn sys_hot_reload_cosmetics(
    time: Res<Time<Real>>,
    mut cfg: ResMut<CosmeticsConfig>,
    mut watch: ResMut<CosmeticsWatch>,
    mut cooldown: Local<f32>,
) {
    *cooldown -= time.delta_secs();
    if *cooldown > 0.0 {
        return;
    }
    *cooldown = POLL_PERIOD_SEC;
    let Ok(mtime) = fs::metadata(GENOME_PATH).and_then(|m| m.modified()) else {
        return;
    };
    if watch.last_mtime == Some(mtime) {
        return;
    }
    watch.last_mtime = Some(mtime);
    let Ok(content) = fs::read_to_string(GENOME_PATH) else {
        return;
    };
    let next = CosmeticsConfig::parse_toml(&content);
    if next == *cfg {
        return;
    }
    *cfg = next;
    watch.reload_count += 1;
    info!("[cosmetics] génome rechargé ({} articles)", cfg.items.len());
}

/// Chemin du catalogue tel qu'il est lu **au runtime**.
pub fn genome_path() -> &'static str {
    GENOME_PATH
}

/// Le catalogue livré, chargé depuis la source — pour les tests.
///
/// Les tests tournent avec le dossier de la CRATE pour répertoire courant, pas
/// la racine du workspace : lire [`genome_path`] tel quel y échoue. Rend `None`
/// sur un checkout partiel — un contrôle qui ne mesure rien n'est pas vert.
pub fn shipped_catalogue() -> Option<CosmeticsConfig> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(GENOME_PATH);
    fs::read_to_string(path)
        .ok()
        .map(|c| CosmeticsConfig::parse_toml(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GENOME: &str = r#"
[[cosmetics]]
id = "crypte_enclume"
kind = "decor"
label = "Le Départ"
palette = "inferno"
source = "start"

[[cosmetics]]
id = "ch3"
kind = "decor"
label = "Chapitre 3"
palette = "festin"
source = "chapter"
chapter = 3

[[cosmetics]]
id = "boutique"
kind = "decor"
label = "Boutique"
palette = "donjon"
source = "shards"
price = 60

[[cosmetics]]
id = "color_azur"
kind = "color"
label = "Azur"
color = "azur"
source = "shards"
price = 40

[[cosmetics]]
id = "hautfait"
kind = "music"
label = "Haut fait"
music = "hub"
source = "exploit"

[shards]
per_round = 2
per_chapter_cleared = 15
"#;

    fn cfg() -> CosmeticsConfig {
        CosmeticsConfig::parse_toml(GENOME)
    }

    #[test]
    fn le_depart_est_toujours_possede() {
        let id = IdentitySave::default();
        let owned = OwnedCosmetics {
            chapters_cleared: 0,
            identity: &id,
        };
        assert!(cfg().get("crypte_enclume").is_some_and(|c| owned.has(c)));
    }

    #[test]
    fn un_article_de_chapitre_se_derive_de_la_progression() {
        let id = IdentitySave::default();
        let c = cfg();
        let ch3 = c.get("ch3").unwrap().clone();
        for (cleared, attendu) in [(2, false), (3, true), (9, true)] {
            let owned = OwnedCosmetics {
                chapters_cleared: cleared,
                identity: &id,
            };
            assert_eq!(owned.has(&ch3), attendu, "cleared={cleared}");
        }
    }

    #[test]
    fn un_article_achete_doit_etre_stocke_meme_a_progression_maximale() {
        let c = cfg();
        let achat = c.get("boutique").unwrap().clone();
        let mut id = IdentitySave::default();
        let owned = OwnedCosmetics {
            chapters_cleared: 999,
            identity: &id,
        };
        assert!(!owned.has(&achat), "finir le jeu ne donne pas les achats");
        id.unlocked_backdrops.push("boutique".into());
        let owned = OwnedCosmetics {
            chapters_cleared: 0,
            identity: &id,
        };
        assert!(owned.has(&achat));
    }

    #[test]
    fn une_couleur_est_possedee_par_son_id_de_couleur_pas_par_l_article() {
        // C'est `unlocked_colors` qui fait foi — le panneau Forgeron le filtre,
        // et le boot y remet `default` si la couleur portée n'y est pas. Stocker
        // l'id d'ARTICLE ferait réinitialiser une couleur achetée au lancement
        // suivant.
        let c = cfg();
        let azur = c.get("color_azur").unwrap().clone();
        let mut id = IdentitySave::default();
        assert!(!OwnedCosmetics {
            chapters_cleared: 0,
            identity: &id
        }
        .has(&azur));

        assert!(grant_and_equip_in_memory(&mut id, &azur));
        assert!(
            id.unlocked_colors.iter().any(|s| s == "azur"),
            "c'est l'id de COULEUR qui doit entrer dans unlocked_colors"
        );
        assert_eq!(id.equipped_color, "azur");
        // 🚨 La vérification qui manquait — et le bug qu'elle aurait attrapé :
        // après l'achat, la POSSESSION doit se voir. `has()` comparait l'id
        // d'article au stock d'ids de couleur → toute couleur payée restait
        // « Débloquer », donc payable deux fois (vu en jeu le 2026-08-06).
        assert!(
            OwnedCosmetics {
                chapters_cleared: 0,
                identity: &id
            }
            .has(&azur),
            "une couleur achetée doit être POSSÉDÉE aux yeux de has()"
        );
        // Deuxième achat impossible : rien à débiter.
        assert!(!grant_and_equip_in_memory(&mut id, &azur));
    }

    #[test]
    fn un_article_sans_charge_utile_est_refuse() {
        // Un décor sans palette, une couleur sans couleur : achetés, ils ne
        // feraient RIEN. On les refuse au parse plutôt que de les vendre.
        let c = CosmeticsConfig::parse_toml(
            r#"
[[cosmetics]]
id = "vide"
kind = "decor"
label = "Vide"
source = "start"

[[cosmetics]]
id = "couleur_sans_couleur"
kind = "color"
label = "Rien"
source = "start"
"#,
        );
        // Les deux refusées → le catalogue retombe sur son miroir.
        assert_eq!(c.items.len(), 1);
        assert_eq!(c.items[0].id, FALLBACK_BACKDROP);
    }

    #[test]
    fn une_source_incomplete_ne_devient_pas_un_cadeau() {
        let c = CosmeticsConfig::parse_toml(
            r#"
[[cosmetics]]
id = "bancal"
kind = "decor"
label = "Bancal"
palette = "inferno"
source = "chapter"

[[cosmetics]]
id = "gratuit_par_accident"
kind = "decor"
label = "Oups"
palette = "donjon"
source = "shards"
"#,
        );
        let id = IdentitySave::default();
        let owned = OwnedCosmetics {
            chapters_cleared: 999,
            identity: &id,
        };
        for key in ["bancal", "gratuit_par_accident"] {
            let item = c.get(key).unwrap();
            assert_eq!(item.source, CosmeticSource::Exploit);
            assert!(!owned.has(item), "« {key} » ne doit pas être offert");
        }
    }

    #[test]
    fn on_ne_rend_jamais_un_decor_non_possede() {
        let c = cfg();
        let id = IdentitySave::default();
        let owned = OwnedCosmetics {
            chapters_cleared: 0,
            identity: &id,
        };
        let rendu = c.resolve_decor("boutique", &owned).unwrap();
        assert_ne!(rendu.id, "boutique");
        assert!(c.resolve_decor("nexiste_pas", &owned).is_some());
    }

    #[test]
    fn les_eclats_recompensent_la_profondeur_pas_le_temps() {
        let r = cfg().shards;
        assert_eq!(r.per_round, 2);
        // Aller plus loin rapporte plus.
        assert!(r.earned(8, false) > r.earned(3, false));
        // Boucler le chapitre ajoute la prime.
        assert_eq!(r.earned(10, true), r.earned(10, false) + 15);
        // Une run nulle ne rapporte rien — pas de revenu de base au lancement.
        assert_eq!(r.earned(0, false), 0);
    }

    #[test]
    fn porter_une_couleur_de_chapitre_la_rend_survivante_au_boot() {
        // Le garde de boot (`sys_init_identity`) remet « default » si la couleur
        // portée n'est pas dans `unlocked_colors`. Une couleur gagnée par
        // chapitre est possédée par DÉRIVATION et n'y entre jamais toute seule :
        // sans inscription à l'équipement, elle est perdue au relancement.
        let cfg = CosmeticsConfig::parse_toml(
            r#"
[[cosmetics]]
id = "color_ch"
kind = "color"
label = "Pourpre"
color = "pourpre"
source = "chapter"
chapter = 4
"#,
        );
        let mut id = IdentitySave::default();
        let item = cfg.get("color_ch").unwrap().clone();
        // Possédée par dérivation dès le chapitre 4 battu.
        assert!(OwnedCosmetics {
            chapters_cleared: 4,
            identity: &id
        }
        .has(&item));
        assert!(!id.unlocked_colors.contains(&"pourpre".to_string()));

        equip_in_memory(&mut id, &item);
        assert_eq!(id.equipped_color, "pourpre");
        assert!(
            id.unlocked_colors.contains(&"pourpre".to_string()),
            "sans ça, le garde de boot la réinitialise à « default »"
        );
        // Idempotent : la porter deux fois ne la duplique pas.
        equip_in_memory(&mut id, &item);
        assert_eq!(
            id.unlocked_colors.iter().filter(|c| *c == "pourpre").count(),
            1
        );
    }

    #[test]
    fn une_couleur_vendue_cesse_d_etre_offerte_au_boot() {
        // 🚨 `sys_init_identity` débloquait TOUTES les couleurs (« MVP :
        // gratuites »). Depuis que le Marketplace en vend, ce cadeau rendrait
        // l'onglet menteur : un prix affiché pour ce que le jeu donne.
        let c = cfg();
        assert!(
            c.color_is_governed("azur"),
            "azur est vendue → plus offerte au boot"
        );
        // Une couleur ABSENTE du catalogue reste gratuite : on ne retire rien
        // par omission.
        assert!(!c.color_is_governed("default"));
        assert!(!c.color_is_governed("une_couleur_jamais_listee"));

        // Et une couleur listée en « start » reste gratuite elle aussi.
        let libre = CosmeticsConfig::parse_toml(
            r#"
[[cosmetics]]
id = "color_libre"
kind = "color"
label = "Libre"
color = "emeraude"
source = "start"
"#,
        );
        assert!(!libre.color_is_governed("emeraude"));
    }

    #[test]
    fn le_catalogue_reel_est_coherent() {
        let Some(cfg) = shipped_catalogue() else {
            eprintln!("génome absent (checkout partiel), contrôle non exécuté");
            return;
        };
        let mut ids: Vec<&str> = cfg.items.iter().map(|c| c.id.as_str()).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "deux articles partagent un id");

        let repli = cfg.get(FALLBACK_BACKDROP).expect("le décor de repli existe");
        assert_eq!(repli.source, CosmeticSource::Start);
        assert_eq!(repli.kind, CosmeticKind::Decor);

        assert!(
            !cfg.items
                .iter()
                .any(|c| c.source == CosmeticSource::Exploit),
            "un article « exploit » est inobtenable tant que les hauts faits n'existent pas"
        );
        // Chaque famille est réellement peuplée : un onglet vide se voit.
        for kind in CosmeticKind::ALL {
            assert!(
                cfg.items.iter().any(|c| c.kind == kind),
                "aucun article pour {kind:?} — l'onglet serait vide"
            );
        }
        for c in &cfg.items {
            assert!(!c.label.is_empty(), "« {} » sans nom affichable", c.id);
        }
    }
}
