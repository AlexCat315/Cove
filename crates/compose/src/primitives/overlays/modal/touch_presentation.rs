//! Translation of Primitives/Overlays/Modal/TouchPresentation.swift.
//!
//! Surface policy for overlays: a sheet on touch surfaces, an anchored panel for
//! pointer input.
use std::rc::Rc;

use cove_core::Environment;
use cove_core::scene::CompositionLocal;

/// Whether overlays present as bottom sheets rather than anchored panels.
pub fn uses_sheet(surface: &Environment) -> bool {
    surface.device_class.is_handheld()
        || (surface.input.contains(cove_core::InputCapabilities::TOUCH)
            && !surface.supports_hover())
}

/// Marks a subtree as rendering inside a touch sheet, so nested menus present in
/// place instead of opening another anchored portal.
pub struct TouchPresentation;
impl TouchPresentation {
    pub fn menu() -> Rc<CompositionLocal<bool>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<bool>> = Rc::new(CompositionLocal::constant(false));
        }
        KEY.with(Rc::clone)
    }

    pub fn is_sheet_menu(node: &cove_core::scene::Node) -> bool {
        node.composition_value(&Self::menu())
    }
}
