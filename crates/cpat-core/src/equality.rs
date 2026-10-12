use crate::pattern::Pattern;

impl Pattern {
    /// v12 `isEqual`: the same kind of pattern with equal children, in
    /// order. Names don't count, except a reference's (it is its name); a
    /// literal compares its token and a regex its source.
    ///
    /// A choice compares its alternatives as given, which is what v12
    /// compares once both are built. A bounded repeat compares v12's slots:
    /// its bound, divider and trimming decide how many there are.
    pub fn is_equal(&self, other: &Pattern) -> bool {
        match (self, other) {
            (Pattern::Literal(a), Pattern::Literal(b)) => a.token() == b.token(),
            (Pattern::Regex(a), Pattern::Regex(b)) => a.source() == b.source(),
            (Pattern::Reference(a), Pattern::Reference(b)) => a.meta.name == b.meta.name,
            (Pattern::Literal(_) | Pattern::Regex(_) | Pattern::Reference(_), _) => false,
            (Pattern::Repeat(a), Pattern::Repeat(b)) if a.slots() != b.slots() => false,
            _ => {
                self.type_name() == other.type_name()
                    && self.child_patterns().count() == other.child_patterns().count()
                    && self
                        .child_patterns()
                        .zip(other.child_patterns())
                        .all(|(a, b)| a.is_equal(b))
            }
        }
    }

    /// v12's `type`: what kind of pattern this is, as JS names it. A choice
    /// is v12's `expression`; a repeat reports its strategy, as v12's
    /// `Repeat` reports its inner repeat's type.
    pub fn type_name(&self) -> &'static str {
        match self {
            Pattern::Literal(_) => "literal",
            Pattern::Regex(_) => "regex",
            Pattern::Sequence(_) => "sequence",
            Pattern::Choice(_) => "expression",
            Pattern::Repeat(repeat) => match repeat.max() {
                None => "infinite-repeat",
                Some(_) => "finite-repeat",
            },
            Pattern::Optional(_) => "optional",
            Pattern::Not(_) => "not",
            Pattern::Cut(_) => "cut",
            Pattern::Reference(_) => "reference",
            Pattern::Context(_) => "context",
            Pattern::RightAssociated(_) => "right-associated",
            Pattern::TakeUntil(_) => "take-until",
            Pattern::Block(_) => "block",
            Pattern::Custom(_) => "custom",
        }
    }
}
