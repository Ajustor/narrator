#[allow(dead_code)]
mod protocole;

use dioxus::prelude::*;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        div { "Narrateur — app_desktop (placeholder)" }
    }
}
