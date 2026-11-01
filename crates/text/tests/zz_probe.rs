use cove_core::FontWeight;
use cove_text::{FontProvider, SystemFontDefaults};
#[test]
fn probe() {
    let mut p = FontProvider::new(14.0, 1.0, 1);
    let name = SystemFontDefaults::primary_font_name();
    let _ = p.load_primary_font(&name, FontWeight::Regular, false);
    let regular = p
        .resolve_runs("ab粗体")
        .last()
        .map(|r| (r.font.post_script_name.clone(), r.font.id));
    println!("regular last = {regular:?}");
    let _ = p.load_primary_font(&name, FontWeight::Bold, false);
    let bold = p
        .resolve_runs("ab粗体")
        .last()
        .map(|r| (r.font.post_script_name.clone(), r.font.id));
    println!("bold last    = {bold:?}");
    let _ = p.load_primary_font(&name, FontWeight::Regular, false);
    let again = p
        .resolve_runs("ab粗体")
        .last()
        .map(|r| (r.font.post_script_name.clone(), r.font.id));
    println!(
        "again        = {again:?}  (id equal to regular: {:?})",
        again.clone().map(|a| a.1) == regular.map(|r| r.1)
    );
    println!(
        "joined = {:?}",
        p.resolve_runs("ab粗体")
            .iter()
            .map(|r| r.text.as_str())
            .collect::<String>()
    );
}
