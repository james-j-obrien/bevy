use super::*;

struct QueryTerm {}

pub struct QueryBuilder<Q: WorldQuery> {
    terms: Vec<QueryTerm>,
}

impl QueryBuilder {
    pub fn build<Q: WorldQuery>(self) -> QueryState<Q, ()> {
        QueryState::from(self)
    }
}
