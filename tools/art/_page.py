"""Genere la page de presentation du dessin. Jetable, prefixe par _ (non versionne)."""
import base64
import os

HERE = os.path.dirname(os.path.abspath(__file__))


def uri(name):
    with open(os.path.join(HERE, name), "rb") as fh:
        return "data:image/png;base64," + base64.b64encode(fh.read()).decode("ascii")


HTML = """<title>Pepin - les quatre faces</title>
<style>
 :root{color-scheme:light dark;--bg:#efedf1;--surf:#fff;--line:#ccc6d2;--ink:#1f1d2b;--soft:#544f66;--gold:#a67a2c;--stage:#141a2b;
  --f:system-ui,-apple-system,"Segoe UI",sans-serif;--m:ui-monospace,"Cascadia Mono",Consolas,monospace}
 @media(prefers-color-scheme:dark){:root:not([data-theme="light"]){--bg:#111726;--surf:#1a2032;--line:#2f3750;--ink:#dcd7e4;--soft:#a29cb8;--gold:#e0bb63;--stage:#0a0e1c}}
 :root[data-theme="dark"]{--bg:#111726;--surf:#1a2032;--line:#2f3750;--ink:#dcd7e4;--soft:#a29cb8;--gold:#e0bb63;--stage:#0a0e1c}
 *{box-sizing:border-box}
 body{margin:0;background:var(--bg);color:var(--ink);font-family:var(--f);font-size:16px;line-height:1.6}
 .w{max-width:980px;margin:0 auto;padding:clamp(28px,5vw,60px) clamp(18px,4vw,40px) 70px;display:flex;flex-direction:column;gap:42px}
 .eb{font-family:var(--m);font-size:11px;letter-spacing:.18em;text-transform:uppercase;color:var(--gold);margin:0 0 12px}
 h1{font-size:clamp(28px,5vw,42px);line-height:1.05;font-weight:800;letter-spacing:-.03em;margin:0 0 14px;text-wrap:balance}
 .deck{max-width:66ch;color:var(--soft);margin:0;font-size:17px}.deck strong{color:var(--ink);font-weight:600}
 h2{font-size:19px;font-weight:700;margin:0 0 6px}
 .lede{margin:0 0 16px;color:var(--soft);font-size:14px;max-width:70ch}
 .stage{background:var(--stage);border:1px solid var(--line);border-radius:3px;padding:16px;display:block}
 .stage img{width:100%;display:block;image-rendering:pixelated}
 .duo{display:grid;grid-template-columns:1fr;gap:18px}
 @media(min-width:760px){.duo{grid-template-columns:1.15fr .85fr}}
 .sc{overflow-x:auto} table{width:100%;border-collapse:collapse;font-size:14px;min-width:520px}
 th,td{text-align:left;padding:9px 12px;border-bottom:1px solid var(--line);vertical-align:top}
 th{font-family:var(--m);font-size:10.5px;letter-spacing:.12em;text-transform:uppercase;color:var(--soft);font-weight:500}
 td.ref{color:var(--ink);white-space:nowrap}
 .note{background:var(--surf);border:1px solid var(--line);border-left:2px solid var(--gold);border-radius:3px;padding:18px 20px}
 .note p{margin:0 0 10px;color:var(--soft)}.note p:last-child{margin:0}.note strong{color:var(--ink)}
 blockquote{margin:0 0 12px;padding-left:14px;border-left:2px solid var(--gold);color:var(--soft);font-style:italic}
 code{font-family:var(--m);font-size:.86em;background:var(--bg);padding:1px 5px;border-radius:2px;color:var(--ink)}
 a{color:var(--gold)}
 .cap{font-family:var(--m);font-size:11px;color:var(--soft);margin:8px 0 0;letter-spacing:.04em}
</style>
<div class="w">
 <header><p class="eb">Forgia &middot; viewmodel &middot; P&eacute;pin</p>
  <h1>Les quatre faces de P&eacute;pin</h1>
  <p class="deck">Trois passes ajout&eacute;es &mdash; <strong>occlusion de contact</strong>, <strong>sp&eacute;culaire au bord</strong>, <strong>halo &eacute;missif</strong> &mdash; et deux pi&egrave;ces qui viennent du GDD <em>The Spared</em> et non de la fiche&nbsp;: le <strong>c&oelig;ur de braise</strong> et la <strong>jauge de confiance</strong>.</p></header>

 <section><h2>Les quatre faces</h2>
  <p class="lede">Une arme de FPS se juge sur quatre vues, pas une&nbsp;: le flanc vend le design, mais c'est le <strong>dos</strong> qu'on regarde en visant, et c'est lui qui d&eacute;cide si l'arme est jouable. Les quatre partagent les m&ecirc;mes pi&egrave;ces, les m&ecirc;mes rampes et la m&ecirc;me finition &mdash; c'est cette finition commune qui les fait lire comme <em>un objet vu sous quatre angles</em> plut&ocirc;t que comme quatre dessins.</p>
  <div class="stage"><img src="__FACES__" alt="Les quatre faces de Pepin"></div></section>

 <section><h2>Ce que chaque vue sert &agrave; v&eacute;rifier</h2>
  <div class="sc"><table><thead><tr><th>Vue</th><th>&Agrave; quoi elle sert</th><th>Contrainte propre</th></tr></thead><tbody>
   <tr><td class="ref">c&ocirc;t&eacute;</td><td>vend le design &mdash; c'est la vue de la fiche d'arme et du <em>viewmodel</em> hanche</td><td>la seule o&ugrave; le c&oelig;ur de braise et l'oriflamme se voient enti&egrave;rement</td></tr>
   <tr><td class="ref">arri&egrave;re</td><td><strong>la vue de vis&eacute;e</strong> &mdash; celle qu'on regarde le plus longtemps dans une partie</td><td>le cran de mire doit rester <strong>d&eacute;gag&eacute;</strong>&nbsp;: la t&ecirc;te se pose au-dessus de la ligne et n'y mord jamais. Sinon l'arme est jolie et injouable.</td></tr>
   <tr><td class="ref">avant</td><td>ce que voit la cible&nbsp;; sert aussi &agrave; la mire adverse</td><td>composition strictement centr&eacute;e&nbsp;: rien ne doit d&eacute;porter le regard hors de la pierre</td></tr>
   <tr><td class="ref">dessus</td><td>v&eacute;rifie que l'arme a une <strong>&eacute;paisseur</strong></td><td>la seule o&ugrave; l'on voit si les bagues font le tour et si le rail est centr&eacute;. Un flanc seul laisse passer une arme plate.</td></tr>
  </tbody></table></div></section>

 <section><h2>Le flanc en d&eacute;tail</h2>
  <p class="lede">260&nbsp;&times;&nbsp;136&nbsp;px de toile. Palette de 91 entr&eacute;es, toutes <em>g&eacute;n&eacute;r&eacute;es</em> en Oklab depuis 8 couleurs de base relev&eacute;es sur la fiche &mdash; aucune choisie &agrave; l'&oelig;il.</p>
  <div class="stage"><img src="__HERO__" alt="Pepin en pixel art"></div></section>

 <section><h2>Ce que le GDD rend obligatoire</h2>
  <blockquote>&laquo;&nbsp;Les armes sont des &acirc;mes de ma&icirc;tres-forgerons vers&eacute;es dans leurs &oelig;uvres &mdash; c'est pourquoi elles parlent. Le c&oelig;ur de braise de l'arme rougeoie quand elle parle.&nbsp;&raquo;</blockquote>
  <p class="lede">Ce n'est pas une ligne d'ambiance&nbsp;: c'est une <strong>pi&egrave;ce fonctionnelle</strong>. Le hublot d'&acirc;me s'allume &agrave; chaque bark, et sa lueur teinte le laiton alentour. La jauge de confiance &mdash; le gimmick de P&eacute;pin au &sect;5 &mdash; passe du <strong>verre froid</strong> &agrave; la <strong>braise</strong>, les deux mati&egrave;res de la DA oppos&eacute;es sur la m&ecirc;me pi&egrave;ce. Une m&eacute;canique qui ne se voit pas sur l'arme se pilote &agrave; l'aveugle.</p>
  <div class="duo">
   <div><div class="stage"><img src="__STATES__" alt="Trois etats"></div>
    <p class="cap">l'arme se tait &middot; elle prend confiance &middot; elle PARLE</p></div>
   <div><div class="stage"><img src="__HEAD__" alt="Tete de dragon"></div>
    <p class="cap">la t&ecirc;te, reconstruite sur points d'ancrage nomm&eacute;s</p></div>
  </div></section>

 <section><h2>Les trois passes, et le d&eacute;faut que chacune corrige</h2>
  <div class="sc"><table><thead><tr><th>Passe</th><th>R&egrave;gle appliqu&eacute;e</th><th>Ce qui n'allait pas sans elle</th></tr></thead><tbody>
   <tr><td class="ref">occlusion de contact</td><td>tout pixel qui touche une <em>autre</em> mati&egrave;re descend d'un cran</td><td>les pi&egrave;ces se juxtaposaient au lieu de s'embo&icirc;ter &mdash; l'arme se lisait comme un collage</td></tr>
   <tr><td class="ref">sp&eacute;culaire au bord</td><td>sur un cylindre le point brillant se pose sur le <strong>bord</strong>, jamais au milieu&nbsp;; largeur &prop; rugosit&eacute;</td><td>tache centrale = <em>pillow shading</em>&nbsp;: le canon gonflait au lieu de tourner</td></tr>
   <tr><td class="ref">halo &eacute;missif</td><td>chaque mati&egrave;re a sa version <em>teint&eacute;e par la lueur</em>, pr&eacute;calcul&eacute;e en Oklab</td><td>le cristal &eacute;tait une tache violette&nbsp;; il n'<strong>&eacute;clairait</strong> pas son sertissage</td></tr>
   <tr><td class="ref">rebond sous le ventre</td><td>lumi&egrave;re renvoy&eacute;e par le sol, en bande tram&eacute;e</td><td>le bas du canon se fondait dans le cerne, l'arme perdait son &eacute;paisseur</td></tr>
  </tbody></table></div>
  <p class="lede" style="margin-top:14px">Sources&nbsp;: <a href="https://gamedevacademy.org/metallic-pixel-art-tutorial/">GameDev Academy &mdash; metallic pixel art</a> (highlight sur l'ar&ecirc;te, points de r&eacute;flexion multiples) &middot; <a href="https://lospec.com/pixel-art-tutorials/tags/metal">Lospec &mdash; tutoriels m&eacute;tal</a> &middot; <a href="https://www.clipstudio.net/how-to-draw/archives/159970">Clip Studio &mdash; rendu des surfaces m&eacute;talliques</a> (rugueux = tache large et terne, poli = &eacute;troite et vive).</p></section>

 <section><h2>Ce qui a &eacute;t&eacute; mesur&eacute;, pas jug&eacute; &agrave; l'&oelig;il</h2>
  <div class="note">
   <p><strong>La &laquo;&nbsp;fente&nbsp;&raquo; &agrave; la bouche du canon n'existait pas.</strong> Elle se voyait pourtant nettement sur le rendu. Un comptage colonne par colonne a montr&eacute; que la bague est simplement <em>plus haute</em> que le canon&nbsp;: ce que je prenais pour un trou &eacute;tait le fond visible au-dessus et au-dessous d'une pi&egrave;ce pro&eacute;minente. Corriger l'aurait cass&eacute;e.</p>
   <p><strong>La t&ecirc;te a &eacute;t&eacute; refaite sur des points d'ancrage nomm&eacute;s</strong> (<code>SNOUT</code>, <code>BROW</code>, <code>NAPE</code>, <code>HINGE</code>&hellip;) plut&ocirc;t qu'en empilant des polygones ajust&eacute;s au jug&eacute;. La version pr&eacute;c&eacute;dente &eacute;tait exactement &ccedil;a, et rendait une masse brune sans structure&nbsp;: <strong>quand on ne peut pas nommer l'ar&ecirc;te qu'on d&eacute;place, on ne corrige rien &mdash; on remue.</strong></p>
   <p><code>audit()</code> v&eacute;rifie m&eacute;caniquement la sym&eacute;trie du sertissage, la division en nombre d'or et l'alignement de la toile sur la trame de 4&nbsp;px. Il rend <code>[]</code>.</p>
  </div></section>

 <section><h2>Ce que ce dessin n'est pas encore</h2>
  <p class="lede">Honn&ecirc;tet&eacute; de port&eacute;e&nbsp;: <strong>rien de tout ceci n'est branch&eacute; dans le jeu.</strong> Les 48 frames install&eacute;es viennent encore du pipeline GLB, pas de ce dessin. Les quatre vues sont dessin&eacute;es, mais <strong>aucune n'est export&eacute;e en frames</strong> ni r&eacute;f&eacute;renc&eacute;e par <code>viewmodel_arena.toml</code>. Et le lien entre <code>heat</code> et le syst&egrave;me de barks, entre <code>confidence</code> et la jauge de gameplay, reste &agrave; c&acirc;bler &mdash; aujourd'hui ce sont des param&egrave;tres de fonction.</p></section>
</div>"""

page = (
    HTML.replace("__HERO__", uri("_pepin_side.png"))
    .replace("__STATES__", uri("_states.png"))
    .replace("__HEAD__", uri("_head.png"))
    .replace("__FACES__", uri("_faces.png"))
)

dest = os.path.join(
    os.environ["TEMP"], "claude", "c--Users-Antoi-Desktop-Forgia-Rewrite",
    "b5cf98ef-7dc0-4290-96aa-4965b4b77e93", "scratchpad", "pepin_art.html",
)
with open(dest, "w", encoding="utf-8") as fh:
    fh.write(page)
print("ecrit", len(page), "->", dest)
