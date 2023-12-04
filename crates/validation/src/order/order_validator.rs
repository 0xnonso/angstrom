use super::{sim::SimValidation, state::StateValidation};

#[allow(dead_code)]
pub struct OrderValidator<DB> {
    sim:   SimValidation,
    state: StateValidation<DB>
}
