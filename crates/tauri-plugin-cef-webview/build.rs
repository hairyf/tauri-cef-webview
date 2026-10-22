const COMMANDS: &[&str] = &[
    "create",
    "set_rect",
    "set_visible",
    "destroy",
    "eval",
    "insert_css",
    "remove_inserted_css",
    "navigate",
    "reload",
    "open_devtools",
    "call",
    "frame",
    "set_overlay_clip",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
