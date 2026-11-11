pub const CSS: &str = r#"
:root {
    --bg: #161310;
    --bg-panel: #211c17;
    --bg-panel-alt: #2a231c;
    --border: #4a3f2f;
    --accent: #c9a227;
    --accent-dim: #8a721c;
    --text: #e8ddc9;
    --text-dim: #c3b89c;
    --danger: #e2585a;
    --danger-bg: #2a1616;
}

html, body {
    margin: 0;
    height: 100%;
    background: var(--bg);
    color: var(--text);
    font-family: -apple-system, "Segoe UI", sans-serif;
    overflow-y: auto;
}

* {
    box-sizing: border-box;
}

.ecran-centre {
    display: flex;
    align-items: center;
    justify-content: center;
    /* `height: 100vh` figeait ce conteneur à exactement la hauteur de
       l'écran : un formulaire plus grand qu'un petit écran de téléphone
       (ex. Création, avec 6 caractéristiques) débordait alors de façon
       invisible et non scrollable, et l'ouverture du clavier (qui réduit
       la hauteur visible) décalait tout le contenu. `min-height` laisse
       le conteneur grandir avec son contenu tout en centrant quand ça
       rentre, et le `padding` évite que la carte touche les bords. */
    min-height: 100%;
    padding: 1.5rem 1rem;
}

.carte {
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 2rem;
    max-width: 480px;
    width: 100%;
}

@media (max-width: 480px) {
    .carte {
        padding: 1.25rem 1rem;
    }
}

.carte h1 {
    color: var(--accent);
    font-size: 1.4rem;
    margin-top: 0;
    border-bottom: 1px solid var(--border);
    padding-bottom: 0.5rem;
}

.champ {
    margin-bottom: 1rem;
}

.texte-discret {
    color: var(--text-dim);
    font-size: 0.85rem;
    margin-top: 0;
    margin-bottom: 1.25rem;
}

.champ label {
    display: block;
    color: var(--text-dim);
    font-size: 0.85rem;
    margin-bottom: 0.25rem;
}

.champ input, .champ select, .champ textarea {
    width: 100%;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.5rem;
    font-size: 1rem;
    font-family: inherit;
    /* WebKitGTK keeps native form-control chrome (a light background on
       <select>/<input>/<textarea>) unless appearance is explicitly reset —
       without this, the background/color rules above are silently ignored
       on these elements specifically. */
    -webkit-appearance: none;
    appearance: none;
}

.champ textarea {
    resize: vertical;
    min-height: 4.5rem;
}

.champ select {
    background-image: linear-gradient(45deg, transparent 50%, var(--text-dim) 50%),
        linear-gradient(135deg, var(--text-dim) 50%, transparent 50%);
    background-position: calc(100% - 18px) calc(1rem + 2px), calc(100% - 13px) calc(1rem + 2px);
    background-size: 5px 5px, 5px 5px;
    background-repeat: no-repeat;
}

.champ .avertissement {
    color: var(--danger);
    font-size: 0.8rem;
    margin-top: 0.25rem;
}

.bouton-principal {
    background: var(--accent);
    color: var(--bg);
    border: none;
    border-radius: 4px;
    padding: 0.6rem 1.2rem;
    font-size: 1rem;
    font-weight: 600;
    cursor: pointer;
}

.bouton-principal:disabled {
    background: var(--border);
    color: var(--text-dim);
    cursor: not-allowed;
}

.bouton-secondaire {
    background: transparent;
    color: var(--text-dim);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.6rem 1.2rem;
    font-size: 1rem;
    cursor: pointer;
}

.bouton-secondaire:hover {
    border-color: var(--accent-dim);
    color: var(--text);
}

.actions-modale {
    display: flex;
    gap: 0.75rem;
    margin-top: 0.5rem;
}

.ecran-jeu {
    display: flex;
    height: 100vh;
}

.zone-principale {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
}

.en-tete {
    padding: 0.75rem 1rem;
    background: var(--bg-panel);
    border-bottom: 1px solid var(--border);
    display: flex;
    gap: 1rem;
    font-size: 0.95rem;
}

.en-tete .lieu {
    color: var(--accent);
    font-weight: 600;
}

.bouton-retour {
    background: none;
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-dim);
    font-size: 0.85rem;
    padding: 0.3rem 0.6rem;
    cursor: pointer;
}

.bouton-retour:disabled {
    opacity: 0.5;
    cursor: default;
}

.panneau-combat {
    background: var(--danger-bg);
    border-bottom: 1px solid var(--danger);
    padding: 0.5rem 1rem;
}

.panneau-combat h3 {
    margin: 0 0 0.25rem 0;
    color: var(--danger);
    font-size: 0.9rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
}

.ennemi {
    font-size: 0.9rem;
}

.journal {
    flex: 1;
    overflow-y: auto;
    padding: 1rem 1.5rem;
    font-family: Georgia, "Times New Roman", serif;
    font-size: 1.05rem;
    line-height: 1.5;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
}

.journal p {
    margin: 0;
    max-width: 75%;
    padding: 0.6rem 0.9rem;
    border-radius: 4px;
}

/* Récit du narrateur/système : bulle à gauche, comme un interlocuteur
   différent du joueur dans une conversation de chat. Qualifiées par
   `.journal` pour l'emporter sur `.journal p` ci-dessus (même nombre de
   classes, donc plus spécifique grâce à ce sélecteur composé). */
.journal .ligne-recit, .journal .narration-en-cours {
    align-self: flex-start;
    background: var(--bg-panel-alt);
    color: var(--text);
    border-radius: 4px 12px 12px 12px;
}

/* Message du joueur : bulle à droite, dans l'accent de l'app, pour que ses
   propres messages se distinguent d'un coup d'œil du récit. */
.journal .ligne-joueur {
    align-self: flex-end;
    background: var(--accent-dim);
    color: var(--text);
    border-radius: 12px 4px 12px 12px;
}

.journal .avertissement {
    color: var(--danger);
    font-style: italic;
}

/* Geste/action entre astérisques (*ainsi*, voir la convention expliquée au
   modèle dans infra::prompts), côté joueur comme côté narrateur (gestes
   d'un PNJ) — grisé pour se distinguer visuellement de la parole/narration
   ordinaire. `--text-dim` reste plus sombre que le `--text` de la bulle
   qui l'entoure, aussi bien sur le fond du récit que sur celui du joueur. */
.action-texte {
    color: var(--text-dim);
}

.indicateur-attente {
    color: var(--text-dim);
    font-style: italic;
    animation: pulsation-attente 1.5s ease-in-out infinite;
}

@keyframes pulsation-attente {
    0%, 100% { opacity: 0.5; }
    50% { opacity: 1; }
}

/* Indicateur de chargement générique (reprise d'une histoire, création en
   cours) — là où une attente notable (chargement du modèle) n'avait
   jusqu'ici aucun retour visuel. */
.chargement-ligne {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    color: var(--text-dim);
    font-size: 0.9rem;
    margin-bottom: 0.75rem;
}

.spinner {
    display: inline-block;
    width: 1rem;
    height: 1rem;
    flex-shrink: 0;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: rotation-spinner 0.7s linear infinite;
}

@keyframes rotation-spinner {
    to { transform: rotate(360deg); }
}

.ligne-histoire-desactivee {
    opacity: 0.5;
    cursor: default;
}

.zone-saisie {
    display: flex;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    background: var(--bg-panel);
    border-top: 1px solid var(--border);
}

.zone-saisie input {
    flex: 1;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.5rem;
    font-size: 1rem;
}

.zone-saisie input:disabled {
    opacity: 0.5;
}

.panneau-pnj {
    width: 260px;
    flex-shrink: 0;
    background: var(--bg-panel-alt);
    border-left: 1px solid var(--border);
    padding: 1rem;
    overflow-y: auto;
}

.panneau-pnj h3 {
    margin: 0 0 0.5rem 0;
    color: var(--accent);
    font-size: 0.9rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
}

.pnj {
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.5rem;
    margin-bottom: 0.5rem;
    cursor: pointer;
    font-size: 0.9rem;
}

.pnj:hover {
    border-color: var(--accent-dim);
}

.pnj strong {
    color: var(--accent);
}

.ligne-histoire {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    flex-wrap: wrap;
}

.bouton-supprimer {
    background: none;
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-dim);
    font-size: 0.8rem;
    padding: 0.3rem 0.6rem;
    cursor: pointer;
    flex-shrink: 0;
}

.bouton-supprimer:hover {
    color: var(--danger);
    border-color: var(--danger);
}

.confirmation-suppression {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.8rem;
    color: var(--danger);
    flex-wrap: wrap;
}

.bouton-danger {
    background: var(--danger);
    color: var(--bg);
    border: none;
    border-radius: 4px;
    padding: 0.3rem 0.6rem;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
    flex-shrink: 0;
}

.pnj ul {
    margin: 0.5rem 0 0 0;
    padding-left: 1rem;
    font-size: 0.8rem;
    color: var(--text-dim);
}

.barre-progression {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    height: 1rem;
    overflow: hidden;
}

.barre-progression-remplissage {
    background: var(--accent);
    height: 100%;
    transition: width 0.2s ease;
}

.voile-de {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 20;
}

.dialogue-de {
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 2rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
}

.etiquette-de {
    color: var(--accent);
    font-weight: 600;
    font-size: 1.1rem;
    margin: 0;
}

.de-bouton {
    width: 6rem;
    height: 6rem;
    border-radius: 12px;
    background: var(--accent);
    color: var(--bg);
    border: none;
    font-size: 1.3rem;
    font-weight: 700;
    cursor: pointer;
    transition: transform 0.1s ease;
}

.de-bouton:active {
    transform: scale(0.92) rotate(-4deg);
}

.de-resultat {
    width: 6rem;
    height: 6rem;
    border-radius: 12px;
    background: var(--bg);
    border: 2px solid var(--accent);
    color: var(--accent);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 2rem;
    font-weight: 700;
}

.banniere-maj {
    position: fixed;
    left: 0;
    right: 0;
    top: 0;
    z-index: 30;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    flex-wrap: wrap;
    background: var(--bg-panel-alt);
    border-bottom: 1px solid var(--accent-dim);
    padding: 0.6rem 1rem;
    font-size: 0.9rem;
}

.banniere-maj-erreur {
    border-bottom-color: var(--danger);
}

.banniere-maj-texte {
    color: var(--text);
    flex: 1;
    min-width: 0;
}

.banniere-maj-actions {
    display: flex;
    gap: 0.5rem;
    flex-shrink: 0;
}

.banniere-maj-barre {
    flex: 1;
    max-width: 12rem;
}
"#;
