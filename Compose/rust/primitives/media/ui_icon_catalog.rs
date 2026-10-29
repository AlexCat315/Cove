//! Translation of Compose/Sources/CoveUICompose/Primitives/Media/UIIconCatalog.swift.
//!
//! The bundled 24x24 alpha-mask SVG icon set. Rust exposes the same grouped
//! names as snake_case functions returning `BundleImageResource`; the catalog
//! `all()` order matches the Swift declaration order.

use std::path::PathBuf;

use cove_imaging::BundleImageResource;

fn bundle_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Sources/CoveUICompose/Resources")
}

/// Resolve a bundled icon by asset slug.
pub fn asset(name: impl Into<String>) -> BundleImageResource {
    BundleImageResource::svg(name, bundle_root(), Some(PathBuf::from("UIIcons")))
}

macro_rules! icon_group {
    ($module:ident { $($function:ident => $slug:literal),* $(,)? }) => {
        pub mod $module {
            use super::asset;
            use cove_imaging::BundleImageResource;
            $(
                pub fn $function() -> BundleImageResource { asset($slug) }
            )*
        }
    };
}

icon_group!(navigation {
    arrow_down => "arrow-down",
    arrow_down_right => "arrow-down-right",
    arrow_left => "arrow-left",
    arrow_right => "arrow-right",
    arrow_up => "arrow-up",
    arrow_up_right => "arrow-up-right",
    chevron_down => "chevron-down",
    chevron_left => "chevron-left",
    chevron_right => "chevron-right",
    chevron_up => "chevron-up",
    dropdown_chevron_down => "dropdown-chevron-down",
    dropdown_chevron_up => "dropdown-chevron-up",
    chevrons_left => "chevrons-left",
    chevrons_right => "chevrons-right",
    chevrons_up_down => "chevrons-up-down",
    circle_arrow_down => "circle-arrow-down",
    circle_arrow_up => "circle-arrow-up",
    corner_down_right => "corner-down-right",
    expand => "expand",
    fullscreen => "fullscreen",
    maximize => "maximize",
    menu => "menu",
    minimize => "minimize",
    move_right => "move-right",
    refresh => "refresh",
    rotate_ccw => "rotate-ccw",
    rotate_cw => "rotate-cw",
    shrink => "shrink",
});

icon_group!(action {
    check => "check",
    check_check => "check-check",
    clipboard => "clipboard",
    copy => "copy",
    crosshair => "crosshair",
    download => "download",
    external_link => "external-link",
    filter => "filter",
    focus => "focus",
    format => "format",
    format_json => "format-json",
    grip => "grip",
    link => "link",
    minus => "minus",
    more_horizontal => "more-horizontal",
    more_vertical => "more-vertical",
    pencil => "pencil",
    plus => "plus",
    power => "power",
    redo => "redo",
    reset => "reset",
    revert => "revert",
    scissors => "scissors",
    search => "search",
    settings => "settings",
    share => "share",
    sliders => "sliders",
    sort_asc => "sort-asc",
    sort_desc => "sort-desc",
    target => "target",
    trash => "trash",
    undo => "undo",
    unlink => "unlink",
    upload => "upload",
    x => "x",
    zoom_in => "zoom-in",
    zoom_out => "zoom-out",
});

icon_group!(status {
    alert_circle => "alert-circle",
    alert_octagon => "alert-octagon",
    alert_triangle => "alert-triangle",
    bell => "bell",
    bell_off => "bell-off",
    bookmark => "bookmark",
    check_circle => "check-circle",
    eye => "eye",
    eye_off => "eye-off",
    flag => "flag",
    heart => "heart",
    heart_fill => "heart-fill",
    help => "help",
    info => "info",
    loader => "loader",
    lock => "lock",
    shield => "shield",
    shield_check => "shield-check",
    sparkles => "sparkles",
    star => "star",
    star_fill => "star-fill",
    unlock => "unlock",
    x_circle => "x-circle",
    zap => "zap",
});

icon_group!(file {
    archive => "archive",
    file => "file",
    file_code => "file-code",
    file_plus => "file-plus",
    file_text => "file-text",
    folder => "folder",
    folder_open => "folder-open",
    folder_plus => "folder-plus",
    inbox => "inbox",
    package => "package",
    save => "save",
});

icon_group!(editor {
    box_ => "box",
    brackets => "brackets",
    brush => "brush",
    bug => "bug",
    code => "code",
    compass => "compass",
    eraser => "eraser",
    gamepad => "gamepad",
    git_branch => "git-branch",
    git_commit => "git-commit",
    git_compare => "git-compare",
    git_merge => "git-merge",
    hammer => "hammer",
    layers => "layers",
    lightbulb => "lightbulb",
    map_pin => "map-pin",
    pause => "pause",
    play => "play",
    repeat_ => "repeat",
    shuffle => "shuffle",
    skip_back => "skip-back",
    skip_forward => "skip-forward",
    stop => "stop",
    terminal => "terminal",
    wrench => "wrench",
});

icon_group!(layout {
    columns_2 => "columns-2",
    columns_3 => "columns-3",
    dashboard => "dashboard",
    grid => "grid",
    indent => "indent",
    outdent => "outdent",
    panel_bottom => "panel-bottom",
    panel_left => "panel-left",
    panel_right => "panel-right",
    panel_top => "panel-top",
    rows_2 => "rows-2",
    rows_3 => "rows-3",
    sidebar => "sidebar",
    table => "table",
});

icon_group!(data {
    activity => "activity",
    calendar => "calendar",
    chart_bar => "chart-bar",
    chart_line => "chart-line",
    chart_pie => "chart-pie",
    clock => "clock",
    hourglass => "hourglass",
    timer => "timer",
    trending_down => "trending-down",
    trending_up => "trending-up",
});

icon_group!(media {
    camera => "camera",
    image => "image",
    mic => "mic",
    monitor => "monitor",
    music => "music",
    video => "video",
    volume => "volume",
    volume_x => "volume-x",
});

icon_group!(device {
    battery => "battery",
    bluetooth => "bluetooth",
    cloud => "cloud",
    cloud_download => "cloud-download",
    cloud_upload => "cloud-upload",
    cpu => "cpu",
    database => "database",
    hard_drive => "hard-drive",
    moon => "moon",
    server => "server",
    sun => "sun",
    wifi => "wifi",
});

icon_group!(user {
    circle_user => "circle-user",
    user => "user",
    user_plus => "user-plus",
    users => "users",
});

icon_group!(text {
    align_center => "align-center",
    align_left => "align-left",
    align_right => "align-right",
    bold => "bold",
    heading => "heading",
    italic => "italic",
    list => "list",
    list_ordered => "list-ordered",
    pilcrow => "pilcrow",
    type_ => "type",
    underline => "underline",
    wrap_text => "wrap-text",
});

icon_group!(shape {
    at_sign => "at-sign",
    book_open => "book-open",
    circle => "circle",
    circle_dot => "circle-dot",
    circle_minus => "circle-minus",
    circle_plus => "circle-plus",
    dot => "dot",
    globe => "globe",
    hash => "hash",
    home => "home",
    key => "key",
    log_in => "log-in",
    log_out => "log-out",
    mail => "mail",
    message_circle => "message-circle",
    message_square => "message-square",
    paperclip => "paperclip",
    pin => "pin",
    pin_off => "pin-off",
    send => "send",
    square => "square",
    square_check => "square-check",
    tag => "tag",
    triangle => "triangle",
});

/// A browsable catalog entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UIIconEntry {
    pub id: String,
    pub name: String,
    pub category: String,
    pub resource: BundleImageResource,
}

impl UIIconEntry {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        category: impl Into<String>,
        resource: BundleImageResource,
    ) -> Self {
        Self { id: id.into(), name: name.into(), category: category.into(), resource }
    }
}

const CATALOG: &[(&str, &str)] = &[
    ("arrow-down", "Navigation"), ("arrow-down-right", "Navigation"),
    ("arrow-left", "Navigation"), ("arrow-right", "Navigation"),
    ("arrow-up", "Navigation"), ("arrow-up-right", "Navigation"),
    ("chevron-down", "Navigation"), ("chevron-left", "Navigation"),
    ("chevron-right", "Navigation"), ("chevron-up", "Navigation"),
    ("dropdown-chevron-down", "Navigation"), ("dropdown-chevron-up", "Navigation"),
    ("chevrons-left", "Navigation"), ("chevrons-right", "Navigation"),
    ("chevrons-up-down", "Navigation"), ("circle-arrow-down", "Navigation"),
    ("circle-arrow-up", "Navigation"), ("corner-down-right", "Navigation"),
    ("expand", "Navigation"), ("fullscreen", "Navigation"),
    ("maximize", "Navigation"), ("minimize", "Navigation"),
    ("move-right", "Navigation"), ("refresh", "Navigation"),
    ("rotate-ccw", "Navigation"), ("rotate-cw", "Navigation"),
    ("shrink", "Navigation"),
    ("check", "Action"), ("check-check", "Action"), ("clipboard", "Action"),
    ("copy", "Action"), ("crosshair", "Action"), ("download", "Action"),
    ("external-link", "Action"), ("filter", "Action"), ("focus", "Action"),
    ("format", "Action"), ("format-json", "Action"), ("grip", "Action"),
    ("link", "Action"), ("minus", "Action"), ("more-horizontal", "Action"),
    ("more-vertical", "Action"), ("pencil", "Action"), ("plus", "Action"),
    ("power", "Action"), ("redo", "Action"), ("reset", "Action"),
    ("revert", "Action"), ("scissors", "Action"), ("search", "Action"),
    ("settings", "Action"), ("share", "Action"), ("sliders", "Action"),
    ("sort-asc", "Action"), ("sort-desc", "Action"), ("target", "Action"),
    ("trash", "Action"), ("undo", "Action"), ("unlink", "Action"),
    ("upload", "Action"), ("x", "Action"), ("zoom-in", "Action"),
    ("zoom-out", "Action"),
    ("alert-circle", "Status"), ("alert-octagon", "Status"),
    ("alert-triangle", "Status"), ("bell", "Status"), ("bell-off", "Status"),
    ("bookmark", "Status"), ("check-circle", "Status"), ("eye", "Status"),
    ("eye-off", "Status"), ("flag", "Status"), ("heart", "Status"),
    ("heart-fill", "Status"), ("help", "Status"), ("info", "Status"),
    ("loader", "Status"), ("lock", "Status"), ("shield", "Status"),
    ("shield-check", "Status"), ("sparkles", "Status"), ("star", "Status"),
    ("star-fill", "Status"), ("unlock", "Status"), ("x-circle", "Status"),
    ("zap", "Status"),
    ("archive", "File"), ("file", "File"), ("file-code", "File"),
    ("file-plus", "File"), ("file-text", "File"), ("folder", "File"),
    ("folder-open", "File"), ("folder-plus", "File"), ("inbox", "File"),
    ("package", "File"), ("save", "File"),
    ("box", "Editor"), ("brackets", "Editor"), ("brush", "Editor"),
    ("bug", "Editor"), ("code", "Editor"), ("compass", "Editor"),
    ("eraser", "Editor"), ("gamepad", "Editor"), ("git-branch", "Editor"),
    ("git-commit", "Editor"), ("git-compare", "Editor"), ("git-merge", "Editor"),
    ("hammer", "Editor"), ("layers", "Editor"), ("lightbulb", "Editor"),
    ("map-pin", "Editor"), ("pause", "Editor"), ("play", "Editor"),
    ("repeat", "Editor"), ("shuffle", "Editor"), ("skip-back", "Editor"),
    ("skip-forward", "Editor"), ("stop", "Editor"), ("terminal", "Editor"),
    ("wrench", "Editor"),
    ("columns-2", "Layout"), ("columns-3", "Layout"), ("dashboard", "Layout"),
    ("grid", "Layout"), ("indent", "Layout"), ("outdent", "Layout"),
    ("panel-bottom", "Layout"), ("panel-left", "Layout"), ("panel-right", "Layout"),
    ("panel-top", "Layout"), ("rows-2", "Layout"), ("rows-3", "Layout"),
    ("sidebar", "Layout"), ("table", "Layout"),
    ("activity", "Data"), ("calendar", "Data"), ("chart-bar", "Data"),
    ("chart-line", "Data"), ("chart-pie", "Data"), ("clock", "Data"),
    ("hourglass", "Data"), ("timer", "Data"), ("trending-down", "Data"),
    ("trending-up", "Data"),
    ("camera", "Media"), ("image", "Media"), ("mic", "Media"),
    ("monitor", "Media"), ("music", "Media"), ("video", "Media"),
    ("volume", "Media"), ("volume-x", "Media"),
    ("battery", "Device"), ("bluetooth", "Device"), ("cloud", "Device"),
    ("cloud-download", "Device"), ("cloud-upload", "Device"), ("cpu", "Device"),
    ("database", "Device"), ("hard-drive", "Device"), ("moon", "Device"),
    ("server", "Device"), ("sun", "Device"), ("wifi", "Device"),
    ("circle-user", "User"), ("user", "User"), ("user-plus", "User"),
    ("users", "User"),
    ("align-center", "Text"), ("align-left", "Text"), ("align-right", "Text"),
    ("bold", "Text"), ("heading", "Text"), ("italic", "Text"),
    ("list", "Text"), ("list-ordered", "Text"), ("pilcrow", "Text"),
    ("type", "Text"), ("underline", "Text"), ("wrap-text", "Text"),
    ("at-sign", "Shape"), ("book-open", "Shape"), ("circle", "Shape"),
    ("circle-dot", "Shape"), ("circle-minus", "Shape"), ("circle-plus", "Shape"),
    ("dot", "Shape"), ("globe", "Shape"), ("hash", "Shape"),
    ("home", "Shape"), ("key", "Shape"), ("log-in", "Shape"),
    ("log-out", "Shape"), ("mail", "Shape"), ("message-circle", "Shape"),
    ("message-square", "Shape"), ("paperclip", "Shape"), ("pin", "Shape"),
    ("pin-off", "Shape"), ("send", "Shape"), ("square", "Shape"),
    ("square-check", "Shape"), ("tag", "Shape"), ("triangle", "Shape"),
];

/// Every bundled icon, grouped in catalog order.
pub fn all() -> Vec<UIIconEntry> {
    CATALOG
        .iter()
        .map(|(slug, category)| UIIconEntry::new(*slug, *slug, *category, asset(*slug)))
        .collect()
}

/// Case-insensitive search across icon name and category. All whitespace
/// separated terms must match.
pub fn search(query: &str) -> Vec<UIIconEntry> {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|term| term.to_lowercase())
        .collect();
    if terms.is_empty() {
        return all();
    }
    all()
        .into_iter()
        .filter(|entry| {
            let text = format!("{} {}", entry.name, entry.category).to_lowercase();
            terms.iter().all(|term| text.contains(term))
        })
        .collect()
}

/// Distinct categories in catalog order.
pub fn categories() -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    for (_, category) in CATALOG {
        if !result.iter().any(|existing| existing == category) {
            result.push((*category).to_string());
        }
    }
    result
}
