use crate::action_schema::interpreter_ou_repli_libre;
use crate::detection_pnj::interpreter_json_detection_pnj;
use crate::prompts::{
    construire_prompt_classification, construire_prompt_condensation, construire_prompt_detection_pnj,
    construire_prompt_introduction, construire_prompt_lieu_initial, construire_prompt_narration,
};
use application::ports::narrateur::{ContexteResultat, ContexteTour, ErreurNarrateur, Narrateur, PnjDetecte};
use domain::{ActionJoueur, Genre, Personnage, PremisseHistoire};
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{LlamaChatMessage, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use std::num::NonZeroU32;
use std::path::Path;

const TAILLE_CONTEXTE: u32 = 4096;
const TOKENS_MAX_CLASSIFICATION: i32 = 100;
// La génération d'un tour (classification + narration + détection PNJ)
// mesurée en conditions réelles (Qwen2.5-3B-Instruct, CPU de bureau) : la
// narration à elle seule y passait ~11 s pour 2-3 phrases, l'essentiel du
// temps d'un tour — voir `construire_prompt_narration` dans `prompts.rs`,
// resserré à "une ou deux phrases courtes" pour réduire d'autant la
// génération (et accessoirement la longueur affichée au joueur). Ce
// plafond baisse en proportion (300 → 180) mais garde une marge
// généreuse au-delà de 1-2 phrases : un modèle plus léger (ex.
// Qwen2.5-1.5B) suit moins bien la consigne de longueur que ce modèle et
// a besoin de cette marge pour ne pas couper sa dernière phrase en plein
// milieu — `tronquer_phrase_incomplete` reste le filet de sécurité qui
// garantit qu'on n'affiche jamais une phrase visiblement inachevée, quel
// que soit ce plafond.
const TOKENS_MAX_NARRATION: i32 = 180;
const TOKENS_MAX_CONDENSATION: i32 = 150;
const TOKENS_MAX_DETECTION_PNJ: i32 = 100;
const TOKENS_MAX_LIEU_INITIAL: i32 = 40;

/// Coupe `texte` au dernier signe de fin de phrase (`.`, `!`, `?`, `…`,
/// `»`) rencontré, pour ne jamais renvoyer une phrase visiblement
/// inachevée quand la génération a été interrompue par le plafond de
/// tokens plutôt que par le modèle lui-même (voir l'appel dans
/// `generer`). Au pire, ça perd la toute dernière phrase en cours de
/// génération plutôt que d'en afficher la moitié. Si aucun signe de fin
/// de phrase n'est trouvé (texte très court, ou sortie JSON sans
/// ponctuation — voir `generer`), renvoie le texte tel quel plutôt que de
/// le vider entièrement.
fn tronquer_phrase_incomplete(texte: &str) -> &str {
    const TERMINATEURS: [char; 5] = ['.', '!', '?', '…', '»'];
    match texte.rfind(|c: char| TERMINATEURS.contains(&c)) {
        Some(i) => {
            let longueur_caractere = texte[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(0);
            &texte[..i + longueur_caractere]
        }
        None => texte,
    }
}

pub struct LlamaNarrateur {
    backend: LlamaBackend,
    model: LlamaModel,
    ctx_params: LlamaContextParams,
}

impl LlamaNarrateur {
    pub fn charger(chemin_modele: &Path) -> Result<Self, String> {
        let backend = LlamaBackend::init()
            .map_err(|e| format!("échec d'initialisation du backend llama.cpp : {e}"))?;

        let model_params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(&backend, chemin_modele, &model_params).map_err(|e| {
            format!("échec de chargement du modèle {} : {e}", chemin_modele.display())
        })?;

        let threads = std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(4);
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(Some(NonZeroU32::new(TAILLE_CONTEXTE).unwrap()))
            .with_n_batch(TAILLE_CONTEXTE)
            .with_n_threads(threads)
            .with_n_threads_batch(threads);

        Ok(Self { backend, model, ctx_params })
    }

    fn generer(
        &self,
        prompt: &str,
        max_tokens: i32,
        grammaire: Option<&str>,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<String, String> {
        let mut ctx = self
            .model
            .new_context(&self.backend, self.ctx_params.clone())
            .map_err(|e| format!("échec de création du contexte : {e}"))?;

        // Le modèle visé (Qwen2.5-3B-Instruct) est instruction-tuned et
        // attend un prompt formaté avec son chat template (balises
        // <|im_start|>/<|im_end|>), pas du texte brut façon "completion".
        // API vérifiée dans la source installée de llama-cpp-2 0.1.160
        // (cargo registry cache), `src/model.rs` :
        // - `LlamaModel::chat_template(&self, name: Option<&str>) ->
        //   Result<LlamaChatTemplate, ChatTemplateError>` (model.rs:319)
        // - `LlamaChatMessage::new(role: String, content: String) ->
        //   Result<Self, NewLlamaChatMessageError>` (model.rs:100)
        // - `LlamaModel::apply_chat_template(&self, tmpl: &LlamaChatTemplate,
        //   chat: &[LlamaChatMessage], add_ass: bool) ->
        //   Result<String, ApplyChatTemplateError>` (model.rs:541) — `add_ass`
        //   is indeed "add assistant" (add the opening assistant tag so
        //   generation can continue directly), set to `true` here.
        let gabarit_discussion = self
            .model
            .chat_template(None)
            .map_err(|e| format!("échec de récupération du template de discussion : {e}"))?;
        let message = LlamaChatMessage::new("user".to_string(), prompt.to_string())
            .map_err(|e| format!("échec de construction du message de discussion : {e}"))?;
        let prompt_formate = self
            .model
            .apply_chat_template(&gabarit_discussion, &[message], true)
            .map_err(|e| format!("échec d'application du template de discussion : {e}"))?;

        let tokens_prompt = self.model.vocab().tokenize(prompt_formate.as_bytes(), true, true);
        if tokens_prompt.len() + max_tokens as usize > TAILLE_CONTEXTE as usize {
            return Err(format!(
                "prompt + génération ({} + {} tokens) dépasse la taille de contexte ({} tokens)",
                tokens_prompt.len(),
                max_tokens,
                TAILLE_CONTEXTE
            ));
        }
        let mut batch = LlamaBatch::new(tokens_prompt.len().max(1), 1);
        let dernier = (tokens_prompt.len() - 1) as i32;
        for (i, token) in (0_i32..).zip(tokens_prompt.into_iter()) {
            batch
                .add(token, i, &[0], i == dernier)
                .map_err(|e| format!("échec d'ajout au batch : {e}"))?;
        }
        ctx.decode(&mut batch)
            .map_err(|e| format!("échec de décodage du prompt : {e}"))?;

        // Grammar API verified against the real installed llama-cpp-2 0.1.160
        // source (cargo registry cache), not just the brief's sketch:
        // - `llama_cpp_2::json_schema_to_grammar` takes a single `&str`
        //   argument (no trailing bool), returning `llama_cpp_2::Result<String>`
        //   i.e. `Result<String, LlamaCppError>`. It is gated behind the
        //   `common` cargo feature, which is enabled by default for this crate.
        // - `LlamaSampler::grammar(model: &LlamaModel, grammar_str: &str,
        //   grammar_root: &str) -> Result<Self, GrammarError>` matches the
        //   brief's sketch exactly.
        let grammaire_sampler = grammaire
            .map(|g| {
                let gbnf = llama_cpp_2::json_schema_to_grammar(g)
                    .map_err(|e| format!("conversion JSON Schema → grammaire échouée : {e}"))?;
                LlamaSampler::grammar(&self.model, &gbnf, "root")
                    .map_err(|e| format!("grammaire invalide : {e}"))
            })
            .transpose()?;

        let mut chaine = Vec::new();
        if let Some(s) = grammaire_sampler {
            chaine.push(s);
        }
        chaine.push(LlamaSampler::dist(1234));
        chaine.push(LlamaSampler::greedy());
        let mut sampler = LlamaSampler::chain_simple(chaine);

        let mut decoder = encoding_rs::UTF_8.new_decoder();
        let mut sortie = String::new();
        let mut n_cur = batch.n_tokens();
        let n_len = n_cur + max_tokens;
        // Reste à `true` tant que la boucle n'a pas vu de token de fin de
        // génération (EOG) — si elle se termine par épuisement du budget
        // plutôt que par ce `break`, `sortie` peut s'arrêter en plein
        // milieu d'une phrase (voir `tronquer_phrase_incomplete`).
        let mut coupe_par_budget = true;

        while n_cur <= n_len {
            let token = sampler.sample(&ctx, batch.n_tokens() - 1);
            sampler.accept(token);
            if self.model.vocab().is_eog(token) {
                coupe_par_budget = false;
                break;
            }

            let morceau = self.model.vocab().token_to_piece(token, true, None);
            let mut texte = String::with_capacity(
                decoder.max_utf8_buffer_length(morceau.len()).unwrap_or(morceau.len()),
            );
            let _ = decoder.decode_to_string(&morceau, &mut texte, false);
            sur_fragment(&texte);
            sortie.push_str(&texte);

            batch.clear();
            batch
                .add(token, n_cur, &[0], true)
                .map_err(|e| format!("échec d'ajout au batch : {e}"))?;
            n_cur += 1;
            ctx.decode(&mut batch).map_err(|e| format!("échec de décodage : {e}"))?;
        }

        if coupe_par_budget {
            sortie = tronquer_phrase_incomplete(&sortie).to_string();
        }
        Ok(sortie)
    }
}

impl Narrateur for LlamaNarrateur {
    fn proposer_action(&mut self, contexte: &ContexteTour) -> Result<ActionJoueur, ErreurNarrateur> {
        // La génération contrainte par grammaire GBNF (`construire_schema_action`
        // + `json_schema_to_grammar`) a été essayée puis abandonnée ici : elle
        // déclenche de façon systématique et reproductible un crash connu et
        // non résolu du moteur de grammaire de llama.cpp
        // (GGML_ASSERT(!stacks.empty()) dans llama-grammar.cpp — voir par ex.
        // ggml-org/llama.cpp#29715), confirmé indépendant du modèle (identique
        // avec Qwen2.5-3B-Instruct et Llama-3.2-3B-Instruct). La génération
        // reste donc libre ici ; `interpreter_json_action` et `valider_action`
        // (dans `application`) sont les deux lignes de défense contre une
        // sortie JSON invalide ou une cible/un objet inexistant. Le schéma
        // JSON et sa conversion en grammaire restent testés dans ce fichier
        // et dans `action_schema.rs`, au cas où une future version de
        // llama-cpp-2 corrige ce bug.
        //
        // Sans grammaire, le modèle 3B quantisé produit parfois un type
        // d'action hors du schéma attendu (ex. observé en test réel :
        // "je prépare une boule de feu" → {"action": "préparer_sort"},
        // qui n'existe pas) — sans filet, cela faisait échouer tout le
        // tour avec EchecGeneration. Puisqu'il n'existe de toute façon
        // aucune action mécanique pour un sort non supporté, une sortie
        // JSON non reconnue est traitée comme une action libre plutôt
        // que comme une erreur : au pire, l'action est simplement
        // racontée sans effet mécanique, au lieu de bloquer le tour.
        let prompt = construire_prompt_classification(contexte);
        let json_brut = self
            .generer(&prompt, TOKENS_MAX_CLASSIFICATION, None, &mut |_| {})
            .map_err(ErreurNarrateur::EchecGeneration)?;
        // Le modèle n'est jamais chargé de reformuler la saisie du joueur : le
        // schéma JSON pour la variante "libre" ne contient plus de champ
        // "texte" (voir Fix 4), donc on substitue ici la saisie réelle du
        // joueur, verbatim — comme le fait déjà `FauxNarrateur`. Ceci couvre
        // aussi le cas de repli (sortie JSON invalide ou hors schéma) ;
        // voir `interpreter_ou_repli_libre`.
        Ok(interpreter_ou_repli_libre(&json_brut, &contexte.saisie_joueur))
    }

    fn raconter_resultat(
        &mut self,
        contexte: &ContexteResultat,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<String, ErreurNarrateur> {
        let prompt = construire_prompt_narration(contexte);
        self.generer(&prompt, TOKENS_MAX_NARRATION, None, sur_fragment)
            .map_err(ErreurNarrateur::EchecGeneration)
    }

    fn condenser_resumes(&mut self, anciens: &[String]) -> Result<String, ErreurNarrateur> {
        let prompt = construire_prompt_condensation(anciens);
        self.generer(&prompt, TOKENS_MAX_CONDENSATION, None, &mut |_| {})
            .map_err(ErreurNarrateur::EchecGeneration)
    }

    fn detecter_pnj(&mut self, narration: &str, noms_connus: &[String]) -> Result<Option<PnjDetecte>, ErreurNarrateur> {
        let prompt = construire_prompt_detection_pnj(narration, noms_connus);
        let json_brut = self
            .generer(&prompt, TOKENS_MAX_DETECTION_PNJ, None, &mut |_| {})
            .map_err(ErreurNarrateur::EchecGeneration)?;
        interpreter_json_detection_pnj(&json_brut).map_err(|e| ErreurNarrateur::EchecGeneration(e.0))
    }

    fn raconter_introduction(
        &mut self,
        personnage: &Personnage,
        lieu: &str,
        premisse: &PremisseHistoire,
        sur_fragment: &mut dyn FnMut(&str),
    ) -> Result<String, ErreurNarrateur> {
        let prompt = construire_prompt_introduction(personnage, lieu, premisse);
        self.generer(&prompt, TOKENS_MAX_NARRATION, None, sur_fragment)
            .map_err(ErreurNarrateur::EchecGeneration)
    }

    fn generer_lieu_initial(&mut self, genre: Genre, premisse_libre: &str) -> Result<String, ErreurNarrateur> {
        let prompt = construire_prompt_lieu_initial(genre, premisse_libre);
        self.generer(&prompt, TOKENS_MAX_LIEU_INITIAL, None, &mut |_| {})
            .map_err(ErreurNarrateur::EchecGeneration)
    }
}
#[cfg(test)]
mod tests {
    use super::tronquer_phrase_incomplete;
    use crate::action_schema::construire_schema_action;
    use domain::{Caracteristiques, Combat, De, EffetObjet, Ennemi, EtatPartie, Objet, Personnage};

    fn personnage_test(inventaire: Vec<Objet>) -> Personnage {
        Personnage::nouveau(
            "Elowen".into(),
            "Guerrier".into(),
            Caracteristiques {
                force: 12,
                dexterite: 14,
                constitution: 13,
                intelligence: 10,
                sagesse: 10,
                charisme: 8,
            },
            inventaire,
        )
    }

    fn etat_sans_rien() -> EtatPartie {
        EtatPartie {
            personnage: personnage_test(vec![]),
            lieu_actuel: "Clairière".into(),
            combat_en_cours: None,
            resumes: vec![],
            pnj_connus: vec![],
            premisse: Default::default(),
            transcript: vec![],
        }
    }

    /// Vérifie que le schéma JSON produit par `construire_schema_action`
    /// pour un état de jeu donné est effectivement convertible en grammaire
    /// GBNF par `llama_cpp_2::json_schema_to_grammar` — pure conversion de
    /// chaînes, ne nécessite aucun fichier de modèle.
    fn verifier_schema_convertible(etat: &EtatPartie) {
        let schema = construire_schema_action(etat);
        let gbnf = llama_cpp_2::json_schema_to_grammar(&schema.to_string());
        assert!(gbnf.is_ok(), "conversion échouée pour {} : {:?}", schema, gbnf.err());
    }

    #[test]
    fn le_schema_sans_combat_ni_objet_est_convertible_en_grammaire() {
        verifier_schema_convertible(&etat_sans_rien());
    }

    #[test]
    fn le_schema_avec_un_ennemi_vivant_est_convertible_en_grammaire() {
        let mut etat = etat_sans_rien();
        etat.combat_en_cours = Some(Combat {
            ennemis: vec![Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3)],
            initiative_joueur: 10,
            initiatives_ennemis: vec![0],
            joueur_esquive: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        });
        verifier_schema_convertible(&etat);
    }

    #[test]
    fn le_schema_avec_un_objet_consommable_est_convertible_en_grammaire() {
        let mut etat = etat_sans_rien();
        etat.personnage = personnage_test(vec![Objet::potion_de_soin()]);
        verifier_schema_convertible(&etat);
    }

    #[test]
    fn le_schema_avec_combat_actif_et_objet_consommable_est_convertible_en_grammaire() {
        let mut etat = etat_sans_rien();
        etat.personnage = personnage_test(vec![Objet::potion_de_soin()]);
        etat.combat_en_cours = Some(Combat {
            ennemis: vec![
                Ennemi::nouveau("Gobelin".into(), 7, De { faces: 6 }, 12, 2, 3),
                Ennemi::nouveau("Loup".into(), 5, De { faces: 4 }, 11, 3, 2),
            ],
            initiative_joueur: 10,
            initiatives_ennemis: vec![0, 0],
            joueur_esquive: false,
            tour_actuel: 0,
            termine: false,
            vainqueur: None,
        });
        verifier_schema_convertible(&etat);

        // S'assure aussi que le filtre "consommable" du schéma (réutilisé
        // depuis action_schema.rs) exclut bien les armes.
        etat.personnage.inventaire.push(Objet::epee());
        assert!(etat
            .personnage
            .inventaire
            .iter()
            .any(|o| !matches!(o.effet, Some(EffetObjet::Consommable { .. }))));
        verifier_schema_convertible(&etat);
    }

    #[test]
    fn tronquer_phrase_incomplete_coupe_a_la_derniere_ponctuation() {
        let texte = "Le vent se lève. Un bruit retentit au loin et le";
        assert_eq!(tronquer_phrase_incomplete(texte), "Le vent se lève.");
    }

    #[test]
    fn tronquer_phrase_incomplete_garde_un_texte_deja_complet() {
        let texte = "Le vent se lève. Un bruit retentit au loin !";
        assert_eq!(tronquer_phrase_incomplete(texte), texte);
    }

    #[test]
    fn tronquer_phrase_incomplete_reconnait_les_points_de_suspension_et_le_guillemet_fermant() {
        assert_eq!(tronquer_phrase_incomplete("Elle hésite… puis"), "Elle hésite…");
        assert_eq!(tronquer_phrase_incomplete("«Qui va là ?» dit-elle soudain"), "«Qui va là ?»");
    }

    #[test]
    fn tronquer_phrase_incomplete_renvoie_le_texte_tel_quel_sans_aucune_ponctuation() {
        // Cas d'une sortie JSON (pas de phrase) ou d'une coupure si précoce
        // qu'aucun signe de fin de phrase n'a encore été généré : mieux
        // vaut renvoyer le texte complet que de le vider.
        let texte = "{\"action\": \"libre\"}";
        assert_eq!(tronquer_phrase_incomplete(texte), texte);
    }

    #[test]
    fn tronquer_phrase_incomplete_reste_sur_une_frontiere_utf8_valide() {
        // "…" et "»" sont des caractères multi-octets en UTF-8 : couper
        // juste après eux sans tenir compte de leur longueur réelle
        // panique (slice index not on char boundary).
        let texte = "Elle hésite…";
        assert_eq!(tronquer_phrase_incomplete(texte), texte);
    }

    // Mesure manuelle, jamais exécutée en CI (`#[ignore]`) : charge le
    // vrai modèle (NARRATEUR_MODELE) et enchaîne les appels d'un tour
    // normal pour voir où le temps part réellement, plutôt que de
    // deviner — voir les `eprintln!("[timing] ...")` temporaires dans
    // `generer`. Lancer avec :
    // NARRATEUR_MODELE=./modeles/Qwen2.5-3B-Instruct-Q4_K_M.gguf \
    //   cargo test -p infra --release mesure_manuelle_latence_un_tour -- --ignored --nocapture
    #[test]
    #[ignore]
    fn mesure_manuelle_latence_un_tour() {
        use super::LlamaNarrateur;
        use application::ports::narrateur::{ContexteResultat, ContexteTour, Narrateur};

        let chemin = std::env::var("NARRATEUR_MODELE").expect("NARRATEUR_MODELE requis pour cette mesure");
        let mut narrateur = LlamaNarrateur::charger(std::path::Path::new(&chemin)).expect("chargement du modèle");

        let etat = etat_sans_rien();
        let contexte_tour = ContexteTour { etat: etat.clone(), saisie_joueur: "J'observe les alentours".into() };

        eprintln!("=== proposer_action ===");
        let action = narrateur.proposer_action(&contexte_tour).unwrap();

        let contexte_resultat = ContexteResultat {
            etat,
            action: action.clone(),
            resume_mecanique: vec![],
            saisie_joueur: "J'observe les alentours".into(),
        };
        eprintln!("=== raconter_resultat ===");
        let narration = narrateur.raconter_resultat(&contexte_resultat, &mut |_| {}).unwrap();
        eprintln!("=== detecter_pnj ===");
        let _ = narrateur.detecter_pnj(&narration, &[]).unwrap();

        eprintln!("action classée : {:?}", action);
        eprintln!("narration ({} caractères) : {}", narration.len(), narration);
    }
}
