use crate::{AnyView, View, ViewKind};
impl<T: View + Clone + 'static> View for Option<T> {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Structural(
            self.as_ref()
                .map(|v| vec![AnyView::new(v.clone())])
                .unwrap_or_default(),
        )
    }
}
impl<T: View + Clone + 'static> View for Vec<T> {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Structural(self.iter().cloned().map(AnyView::new).collect())
    }
}
