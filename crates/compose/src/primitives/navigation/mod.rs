//! Navigation primitives.
pub(crate) mod navigation_stack;
pub(crate) mod route_outlet;
pub mod stepper;

pub use api::*;

pub(crate) mod api {
    pub use super::navigation_stack::{NavigationStack, navigation_title};
    pub use super::route_outlet::RouteOutlet;
    pub use super::stepper::{StepStatus, Stepper, StepperAxis, StepperItem, StepperOptions};
}
