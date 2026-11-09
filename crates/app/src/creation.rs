use application::ports::generateur_de::GenerateurDe;
use application::usecases::creer_personnage::{
    creer_personnage, tirer_valeurs_caracteristiques, CLASSES_VALIDES,
};
use domain::Personnage;
use std::io::{self, Write};

pub fn creer_personnage_interactif(de: &mut dyn GenerateurDe) -> Personnage {
    println!("=== Création de personnage ===");

    let nom = lire_ligne_non_vide("Nom de ton personnage : ");
    let classe = lire_classe();
    let valeurs = lire_caracteristiques(de);

    creer_personnage(&nom, &classe, valeurs).expect("construction du personnage échouée")
}

fn lire_ligne_non_vide(invite: &str) -> String {
    loop {
        print!("{}", invite);
        io::stdout().flush().ok();
        let mut ligne = String::new();
        io::stdin().read_line(&mut ligne).expect("lecture échouée");
        let ligne = ligne.trim().to_string();
        if !ligne.is_empty() {
            return ligne;
        }
        println!("Le nom ne peut pas être vide.");
    }
}

fn lire_classe() -> String {
    loop {
        println!("Classes disponibles : {}", CLASSES_VALIDES.join(", "));
        print!("Choisis une classe : ");
        io::stdout().flush().ok();
        let mut ligne = String::new();
        io::stdin().read_line(&mut ligne).expect("lecture échouée");
        let choix = ligne.trim().to_string();
        if CLASSES_VALIDES.contains(&choix.as_str()) {
            return choix;
        }
        println!("Classe invalide.");
    }
}

fn lire_caracteristiques(de: &mut dyn GenerateurDe) -> [u32; 6] {
    let noms = ["Force", "Dextérité", "Constitution", "Intelligence", "Sagesse", "Charisme"];
    let tirage = tirer_valeurs_caracteristiques(de);
    println!("Caractéristiques tirées pour cette aventure : {:?}", tirage);
    let mut disponibles: Vec<u32> = tirage.to_vec();
    let mut valeurs = [0u32; 6];

    for (i, nom) in noms.iter().enumerate() {
        loop {
            println!("Valeurs restantes : {:?}", disponibles);
            print!("Valeur pour {} : ", nom);
            io::stdout().flush().ok();
            let mut ligne = String::new();
            io::stdin().read_line(&mut ligne).expect("lecture échouée");
            match ligne.trim().parse::<u32>() {
                Ok(valeur) if disponibles.contains(&valeur) => {
                    disponibles.retain(|&v| v != valeur);
                    valeurs[i] = valeur;
                    break;
                }
                _ => println!("Choisis une valeur parmi celles restantes."),
            }
        }
    }

    valeurs
}
