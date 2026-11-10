// Grave le hash court du commit courant dans le binaire via la variable
// d'environnement de compilation `NARRATEUR_GIT_HASH`, lue par
// `mise_a_jour::VERSION_ACTUELLE`. `git` peut être absent du PATH (certains
// environnements de build CI/Android) ou le répertoire de build peut ne pas
// être un dépôt git (archive source, build hors arbre) : dans ces deux cas
// on retombe sur la chaîne littérale "inconnu" plutôt que de faire échouer
// toute la compilation, ce script tournant à chaque build (desktop compris).
fn main() {
    let hash = commande_git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "inconnu".to_string());

    println!("cargo:rustc-env=NARRATEUR_GIT_HASH={hash}");

    emettre_rerun_if_changed();
}

/// Sans directive `cargo:rerun-if-changed` explicite, Cargo retombe sur son
/// comportement par défaut : ne relancer ce script que si un fichier DANS
/// `crates/app_desktop/` change. Or la plupart des commits de ce projet ne
/// touchent que `crates/domain`, `crates/infra` ou `crates/application`
/// (la logique de jeu) — `cargo build` recompile alors correctement
/// `app_desktop` (qui en dépend), mais sans jamais relancer CE script, donc
/// `NARRATEUR_GIT_HASH` reste gravé sur l'ancien commit. Conséquence
/// concrète : une APK reconstruite après un tel commit embarque un hash
/// périmé, qui ne correspondra jamais à celui d'un `version.json` à jour —
/// la bannière de mise à jour s'affiche après chaque "mise à jour" installée,
/// indéfiniment.
///
/// On émet donc explicitement les fichiers dont un changement doit
/// invalider ce script : `HEAD`, la référence qu'il pointe (la branche
/// courante), et `packed-refs` (où une référence peut finir après un
/// `git gc`). Ce projet travaille depuis un *worktree* git, où `.git` à la
/// racine est un FICHIER pointant vers le vrai répertoire git (pas un
/// répertoire) — `git rev-parse --git-path` résout ça correctement, un
/// chemin en dur comme `.git/HEAD` donnerait un chemin inexistant.
///
/// Si `git` est absent ou que le répertoire n'est pas un dépôt (mêmes cas
/// que ci-dessus), ou si la résolution d'une référence échoue (HEAD détaché
/// par exemple), on n'émet simplement pas la directive correspondante — pas
/// de panique : le comportement par défaut de Cargo (moins précis mais pas
/// absent) continue alors de s'appliquer pour cette pièce manquante.
///
/// Autre piège documenté par Cargo lui-même : une directive
/// `rerun-if-changed` pointant vers un fichier qui n'existe PAS DU TOUT fait
/// relancer le script à CHAQUE build, inconditionnellement (Cargo ne peut
/// pas détecter l'apparition du fichier autrement). `packed-refs` n'existe
/// que si `git gc`/`pack-refs` a déjà tourné une fois sur ce dépôt — ce
/// n'est pas le cas ici, par exemple. `git_path_existant` ne retient donc
/// que les chemins déjà présents sur disque au moment du build.
fn emettre_rerun_if_changed() {
    if let Some(chemin) = git_path_existant("HEAD") {
        println!("cargo:rerun-if-changed={chemin}");
    }

    if let Some(chemin) = git_path_existant("packed-refs") {
        println!("cargo:rerun-if-changed={chemin}");
    }

    // La branche courante (ex. `refs/heads/main`) : son fichier de référence
    // change à chaque nouveau commit sur cette branche, même sans qu'on
    // change de HEAD.
    if let Some(reference) = commande_git(&["rev-parse", "--symbolic-full-name", "HEAD"]) {
        if let Some(chemin) = git_path_existant(&reference) {
            println!("cargo:rerun-if-changed={chemin}");
        }
    }
}

/// Résout un chemin interne au dépôt (relatif au répertoire `.git` réel,
/// pas forcément `.git/` lui-même — voir `emettre_rerun_if_changed`) via
/// `git rev-parse --git-path`, seule méthode fiable sous un worktree — et
/// ne le retourne que s'il existe déjà sur disque (voir la note sur
/// `rerun-if-changed` ci-dessus).
fn git_path_existant(chemin_relatif: &str) -> Option<String> {
    let chemin = commande_git(&["rev-parse", "--git-path", chemin_relatif])?;
    std::path::Path::new(&chemin).exists().then_some(chemin)
}

/// Exécute `git` avec `args` et retourne sa sortie standard (recadrée,
/// jamais vide) si la commande a pu être lancée et a réussi — `None` dans
/// tous les autres cas (git absent, pas un dépôt, commande en échec).
fn commande_git(args: &[&str]) -> Option<String> {
    std::process::Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|sortie| sortie.status.success())
        .and_then(|sortie| String::from_utf8(sortie.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
