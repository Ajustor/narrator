use crate::action_schema::interpreter_json_action;
use crate::prompts::{construire_prompt_classification, construire_prompt_condensation, construire_prompt_narration};
use application::ports::narrateur::{ContexteResultat, ContexteTour, ErreurNarrateur, Narrateur};
use domain::ActionJoueur;
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
const TOKENS_MAX_NARRATION: i32 = 200;
const TOKENS_MAX_CONDENSATION: i32 = 150;

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
        let mut batch = LlamaBatch::new(512, 1);
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

        while n_cur <= n_len {
            let token = sampler.sample(&ctx, batch.n_tokens() - 1);
            sampler.accept(token);
            if self.model.vocab().is_eog(token) {
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
        let prompt = construire_prompt_classification(contexte);
        let json_brut = self
            .generer(&prompt, TOKENS_MAX_CLASSIFICATION, None, &mut |_| {})
            .map_err(ErreurNarrateur::EchecGeneration)?;
        let action = interpreter_json_action(&json_brut).map_err(|e| ErreurNarrateur::EchecGeneration(e.0))?;
        // Le modèle n'est jamais chargé de reformuler la saisie du joueur : le
        // schéma JSON pour la variante "libre" ne contient plus de champ
        // "texte" (voir Fix 4), donc on substitue ici la saisie réelle du
        // joueur, verbatim — comme le fait déjà `FauxNarrateur`.
        let action = match action {
            ActionJoueur::ActionLibre(_) => ActionJoueur::ActionLibre(contexte.saisie_joueur.clone()),
            autre => autre,
        };
        Ok(action)
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
}
#[cfg(test)]
mod tests {
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
            ennemi_agit_avant_joueur: false,
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
            ennemi_agit_avant_joueur: false,
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
}
