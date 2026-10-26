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
}

* {
    box-sizing: border-box;
}

.ecran-centre {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
}

.carte {
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 2rem;
    max-width: 480px;
    width: 100%;
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

.champ label {
    display: block;
    color: var(--text-dim);
    font-size: 0.85rem;
    margin-bottom: 0.25rem;
}

.champ input, .champ select {
    width: 100%;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.5rem;
    font-size: 1rem;
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
}

.journal p {
    margin: 0 0 0.75rem 0;
}

.journal .narration-en-cours {
    color: var(--text);
}

.journal .avertissement {
    color: var(--danger);
    font-style: italic;
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

.pnj ul {
    margin: 0.5rem 0 0 0;
    padding-left: 1rem;
    font-size: 0.8rem;
    color: var(--text-dim);
}
"#;
