# Digest des logs et capteurs — RÈGLE BLOQUANTE

> **Ne JAMAIS lire `forgia2_run.log` ni les 97 `forgia2_*.json` en entier.**
> Passer par `tools/ai/forgia_digest.py`. Mesuré le 2026-08-07 :
> **30 768 o → 2 558 o (−92 %)**, et le signal en ressort *plus* lisible.

---

## 1. Pourquoi

Un log de run fait ~30 Ko pour ~190 lignes, dont la moitié répète le même
avertissement à la frame près et 106 sont du bruit de démarrage vu une seule
fois. Le lire en entier coûte cher **et noie le symptôme** : cinq alertes
d'armure identiques se lisent comme cinq problèmes alors que c'en est un.

`rtk log` compresse déjà, mais il garde les codes ANSI, tronque le message et
**jette les lignes INFO** — or dans Forgia le signal y est presque toujours
(`[arena-backdrop] 17 props`, `[avatar] rebranchée`, `[cosmetics] +20 Éclats`).

---

## 2. Le principe : réduire, point.

La réduction est **déterministe** — regex, normalisation, regroupement, comptage.
Gratuite, exacte, instantanée, reproductible. C'est 100 % du gain.

Un maillon « interprétation par un modèle local » (Hermes 3) a été essayé le
2026-08-07 puis **retiré le jour même** : sur le premier cas réel il a
paraphrasé le digest en moins précis, écrivant « 21h31 » là où le log dit
21:19:31. Sur un bug de cycle de vie qui se joue à la milliseconde, cette
approximation envoie chercher au mauvais endroit.

**Leçon, plus large que l'outil** : résumer une donnée exacte avec un modèle
approximatif est une RÉGRESSION, pas un service. Le digest se cite ; un résumé
de modèle ne se cite pas. Ne pas réintroduire cette couche sans une raison
mesurée.

## 3. Commandes

```bash
python tools/ai/forgia_digest.py all              # défaut : log + capteurs
python tools/ai/forgia_digest.py log --module avatar
python tools/ai/forgia_digest.py sensors          # 97 capteurs → 540 o
python tools/ai/forgia_digest.py log --tout       # rouvre le démarrage replié
python tools/ai/forgia_digest.py all --ask "pourquoi l'armure est figée ?"
```

**Quand l'user dit « regarde »** : commencer par `forgia_digest.py all`. Il rend
les capteurs en alerte ET les motifs saillants du log en une lecture. N'ouvrir
un fichier brut qu'ensuite, et seulement celui que le digest désigne.

---

## 4. Ce que le digest apporte que le brut n'a pas

- **Le COMPTE** — `x5` sur une alerte dit « une cause, cinq pièces », pas cinq bugs.
- **La FENÊTRE horaire** — `21:19:15→21:19:31` sur un rebranchement, avec une
  alerte 3 ms plus tard, *est* le diagnostic d'un bug de cycle de vie. C'est
  exactement ce qui a résolu l'armure figée le 2026-08-06.
- **Le repli du démarrage** — 106 lignes de boot comptées, pas lues.

---

## 5. Ce que cet outil ne fait pas

Il ne diagnostique pas. Il RANGE, pour que le diagnostic soit possible en une
lecture. Les défauts réels de ce codebase — l'`Area` egui plafonnée à 400 px,
un garde d'autosave désarmé — se trouvent en raisonnant sur ce qu'il montre,
pas en lui demandant.

---

## 6. Cross-refs

- `bug-triage.md` — « Grep > Read sur tout fichier > 5 KB ». Ce digest est
  l'outil qui applique cette règle aux logs.
- `observability-required.md` — les capteurs existent ; encore faut-il les lire
  sans se ruiner.
- `model-selection.md` — ne pas payer le palier haut pour du mécanique. Ici on
  ne paie AUCUN palier : la réduction est du code.

---

*Adoptée 2026-08-07. Origine : une session où le même log de 30 Ko a été grepé
huit fois de suite pour en extraire trois lignes. La tentative d'y adjoindre un
modèle local a été retirée le jour même — cf. §2.*
