// Grave le hash court du commit courant dans le binaire via la variable
// d'environnement de compilation `NARRATEUR_GIT_HASH`, lue par
// `mise_a_jour::VERSION_ACTUELLE`. `git` peut être absent du PATH (certains
// environnements de build CI/Android) ou le répertoire de build peut ne pas
// être un dépôt git (archive source, build hors arbre) : dans ces deux cas
// on retombe sur la chaîne littérale "inconnu" plutôt que de faire échouer
// toute la compilation, ce script tournant à chaque build (desktop compris).
fn main() {
    let hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|sortie| sortie.status.success())
        .and_then(|sortie| String::from_utf8(sortie.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "inconnu".to_string());

    println!("cargo:rustc-env=NARRATEUR_GIT_HASH={hash}");
}
