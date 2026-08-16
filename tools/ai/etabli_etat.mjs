#!/usr/bin/env node
/**
 * Extrait l'etat de synthese de l'Etabli Forgia, en JSON.
 *
 * Pourquoi ce script existe
 * -------------------------
 * Le recap Telegram doit afficher l'avancement du moteur et du jeu. La tentation
 * serait de recopier les chiffres dans le script d'envoi — ce serait « une grandeur
 * ecrite deux fois », la classe de defaut n°1 du projet : les deux copies finissent
 * toujours par diverger, et on ne sait plus laquelle ment.
 *
 * Ici, la SOURCE UNIQUE reste `docs/etabli/etabli-forgia.html`. Ce script en lit
 * les blocs de donnees et les evalue tels quels. Un chiffre corrige dans l'Etabli
 * part automatiquement sur le telephone ; aucun n'est saisi deux fois.
 *
 * La jauge moteur est DERIVEE de l'audit des capacites (une partielle vaut moitie),
 * exactement comme la page la calcule. La jauge jeu est la ponderation des phases.
 *
 * Usage : node tools/ai/etabli_etat.mjs [chemin/vers/etabli.html]
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const RACINE = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const CHEMIN = process.argv[2] || join(RACINE, 'docs', 'etabli', 'etabli-forgia.html');

const html = readFileSync(CHEMIN, 'utf8');
const script = html.match(/<script>([\s\S]*?)<\/script>/);
if (!script) { console.error('[etabli] aucun bloc <script> trouve'); process.exit(2); }

// Le code au-dela de ce marqueur touche le DOM et localStorage : il ne s'evalue
// pas hors navigateur. Tout ce qui precede est de la donnee pure.
const COUPE = "const KEY=";
const i = script[1].indexOf(COUPE);
if (i < 0) { console.error(`[etabli] marqueur de coupe "${COUPE}" introuvable`); process.exit(2); }

let D;
try {
  D = new Function(script[1].slice(0, i) + '\nreturn {CAPS, SYS, MESURE, STORIES, VEILLE, VEILLE_MAJ};')();
} catch (e) {
  console.error('[etabli] evaluation impossible :', e.message);
  process.exit(2);
}

// `&amp;` etc. viennent du HTML de la page — le telephone veut du texte.
const txt = s => String(s).replace(/<[^>]+>/g, '')
  .replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&nbsp;/g, ' ');

const n = (a, e) => a.filter(c => c.e === e).length;
const pond = a => { const w = a.reduce((s, x) => s + x.w, 0) || 1;
                    return Math.round(a.reduce((s, x) => s + x.w * x.p, 0) / w); };

const ouverte = D.MESURE.dette.filter(d => !d.fait);
const risque = r => ouverte.filter(d => d.risque === r).length;

console.log(JSON.stringify({
  moteur: {
    pct: Math.round((n(D.CAPS, 'prod') + n(D.CAPS, 'part') * 0.5) / D.CAPS.length * 100),
    prod: n(D.CAPS, 'prod'), part: n(D.CAPS, 'part'), abs: n(D.CAPS, 'abs'), total: D.CAPS.length,
    absentes: D.CAPS.filter(c => c.e === 'abs').map(c => txt(c.n)),
    chantiers: D.MESURE.moteur.map(c => ({ n: c.n, t: txt(c.t), p: c.p, gate: txt(c.gate) }))
  },
  jeu: {
    pct: pond(D.MESURE.phases),
    phases: D.MESURE.phases.map(p => ({ n: p.n, t: txt(p.t), p: p.p })),
    prod: n(D.SYS, 'prod'), part: n(D.SYS, 'part'), abs: n(D.SYS, 'abs'), total: D.SYS.length,
    absents: D.SYS.filter(s => s.e === 'abs').map(s => txt(s.n))
  },
  dette: { ouverte: ouverte.length, soldee: D.MESURE.dette.length - ouverte.length,
           haut: risque('haut'), moyen: risque('moyen'), bas: risque('bas'),
           top: ouverte.filter(d => d.risque === 'haut').map(d => txt(d.ti)) },
  wip: D.MESURE.now.map(s => ({ id: s.id, s: s.s, t: txt(s.t) })),
  veille: { total: D.VEILLE.length, maj: D.VEILLE_MAJ }
}, null, 1));
