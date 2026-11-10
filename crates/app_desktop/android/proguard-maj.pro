# Mise à jour Android (`mise_a_jour::lancer_installation`) : la méthode
# statique `FileProvider.getUriForFile` n'est appelée QUE par JNI depuis
# Rust, donc invisible pour R8 — sans cette règle, un build release
# (`isMinifyEnabled = true`) la supprimerait ou la renommerait, et l'appel
# JNI échouerait avec `NoSuchMethodError`. La classe elle-même est déjà
# conservée par la règle qu'AAPT génère pour le `<provider>` du manifeste,
# mais pas ses méthodes statiques.
-keep class androidx.core.content.FileProvider {
    public static android.net.Uri getUriForFile(android.content.Context, java.lang.String, java.io.File);
}
