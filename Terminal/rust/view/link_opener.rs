//! Default handling for Cmd-clicked OSC 8 hyperlinks: hand them to the system.
//!
//! Programs choose these URIs, so only web, mail and file links are accepted.

/// Schemes a program is allowed to open on the user's behalf.
pub const SCHEMES: [&str; 4] = ["http", "https", "mailto", "file"];

/// Validates a link the program printed, or `None` when the scheme is not one a
/// terminal may open.
pub fn url(link: &str) -> Option<String> {
    let link = link.trim();
    if link.is_empty() {
        return None;
    }
    let (scheme, rest) = link.split_once(':')?;
    let scheme = scheme.to_ascii_lowercase();
    if !SCHEMES.contains(&scheme.as_str()) || rest.is_empty() {
        return None;
    }
    Some(link.to_owned())
}

/// Hands a validated link to the platform's default handler.
pub fn open(link: &str) {
    let Some(url) = url(link) else { return };
    #[cfg(target_os = "macos")]
    let (program, arguments): (&str, Vec<&str>) = ("open", vec![&url]);
    #[cfg(target_os = "windows")]
    let (program, arguments): (&str, Vec<&str>) = ("rundll32", vec!["url.dll,FileProtocolHandler", &url]);
    #[cfg(all(unix, not(target_os = "macos")))]
    let (program, arguments): (&str, Vec<&str>) = ("xdg-open", vec![&url]);

    let _ = std::process::Command::new(program).args(arguments).spawn();
}
