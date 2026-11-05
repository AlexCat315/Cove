//! Independent application: only cove-media is declared, no build script or patch.
use cove_media::*;

fn ffmpeg_is_linked() -> Result<(), Box<dyn std::error::Error>> {
    let context = MediaContext::new(ContextOptions::default())?;
    assert!(!context.backend_version().is_empty());
    assert!(context.has_decoder("h264"));
    assert!(context.has_decoder("aac"));
    let player = context.create_player(PlayerOptions::default())?;
    player.close()?;
    drop(player);
    context.shutdown();
    context.wait_closed(std::time::Duration::from_secs(5))?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    ffmpeg_is_linked()
}

#[test]
fn an_external_application_can_use_the_bundled_ffmpeg() {
    ffmpeg_is_linked().expect("external consumer");
}
