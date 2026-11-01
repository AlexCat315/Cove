use crate::{AnyView, EmptyView, SourceLocatedView, TupleView, View};
pub struct ViewBuilder;
impl ViewBuilder {
    #[track_caller]
    pub fn expression(view: impl View + 'static) -> SourceLocatedView {
        SourceLocatedView::expression(view)
    }
    pub fn empty() -> EmptyView {
        EmptyView
    }
    pub fn block(views: Vec<AnyView>) -> TupleView {
        TupleView::new(views)
    }
    pub fn optional(view: Option<AnyView>) -> TupleView {
        TupleView::new(view.into_iter().collect())
    }
    pub fn either(
        first: bool,
        left: impl FnOnce() -> AnyView,
        right: impl FnOnce() -> AnyView,
    ) -> AnyView {
        if first { left() } else { right() }
    }
    pub fn array(views: Vec<AnyView>) -> TupleView {
        TupleView::new(views)
    }
}
#[macro_export]
macro_rules! views{($($view:expr),*$(,)?)=>{$crate::TupleView::new(vec![$($crate::AnyView::new($crate::ViewBuilder::expression($view))),*])};}
