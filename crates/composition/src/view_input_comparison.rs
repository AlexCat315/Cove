use crate::View;
pub(crate) struct ViewInputComparison;
impl ViewInputComparison {
    pub fn equal(lhs: &dyn View, rhs: &dyn View) -> bool {
        lhs.as_any().type_id() == rhs.as_any().type_id() && lhs.same_inputs(rhs)
    }
}
