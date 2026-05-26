# Forgia — la bible (version courte)

## L'histoire en 3 phrases

> **Un méchant a volé les âmes des armes. Toi, tu es un apprenti gentil. Tu les libères une par une, elles deviennent tes amies, et elles n'arrêtent pas de parler.**

C'est tout. Si un enfant de 8 ans comprend ça, on a gagné.

## Le ton

- **Mignon avant tout.** Couleurs vives, formes rondes, personnages expressifs (Overwatch + Pixar).
- **Drôle dans le moment.** L'humour vient des armes qui disent des trucs absurdes pendant qu'on tire (Boucherie qui parle de viande, Lenoir qui se plaint de ses manchettes).
- **Pas de méchanceté gratuite.** Le méchant est ridicule autant qu'inquiétant (genre Bowser, Capitaine Crochet, l'Empereur Zurg).
- **Pas de tragédie adulte.** Personne ne meurt à l'écran. Les âmes "dorment" quand on les libère, on ne tue pas.
- **Le héros est doux.** L'Apprenti parle peu, sourit beaucoup, encourage ses armes.

## Règles d'écriture (les vraies)

| Règle | Pourquoi |
|---|---|
| **Une voiceline = moins de 5 secondes** | Lisible enfants, support bubble UI |
| **Vocabulaire CE2** (8 ans) | Mots simples. Pas de "consternation", oui à "Ah ben mince" |
| **Jamais de gros mots** | *"Crénom !" / "Saperlipopette !" / "Sapristi !"* à la place |
| **Pas de sang, pas de "tuer"** | On dit *"il dort" / "il a fait dodo" / "il est parti"* |
| **Le héros parle peu** | Les armes jacasses. Toi tu acquiesces. (Style Hadès inverse : c'est l'équipement qui cause) |
| **Chaque persona = 1 obsession + 1 tic de langage** | Pour qu'on reconnaisse direct qui parle, même les yeux fermés |

## Structure du dossier

```
docs/lore/
├── README.md         ← (ce fichier) pitch + ton
├── monde.md          ← l'univers en 1 page (court)
├── personas/
│   ├── pepin.md      ← 🔓 timide MVP
│   ├── bourrasque.md ← 🔓 pétillante MVP
│   ├── lenoir.md     ← 🔓 snob MVP
│   ├── boucherie.md  ← 🔓 brutal joyeux MVP
│   ├── apprenti.md   ← 🦸 héros (parle peu)
│   └── forgeron_noir.md ← 👹 méchant ridicule
└── locations/
    └── crypts_of_anvil.md ← 🌋 arène polish target
```

(Pas de `bible.md` géante. Pas de `mythology.md` dense. Pas de factions. Pas de timeline. Si ça tient pas en 1 page, c'est trop.)

## Cross-refs code

| Lore | Code/Asset |
|---|---|
| Voicelines (~100 lignes FR déjà écrites) | `assets/genomes/roguelite/roguelite_dialogue.toml` |
| Stats armes | `assets/genomes/roguelite/roguelite_weapons.toml` |
| Stats méchant | `assets/genomes/roguelite/roguelite_enemies.toml` (Le Forgeron Noir) |
| Arène cible | `assets/genomes/roguelite_stages.toml` (Crypts of Anvil) |
| Pipeline bark | `crates/forgia-audio/` (à finaliser) |
