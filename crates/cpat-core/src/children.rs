use crate::pattern::Pattern;

/// A pattern's children as an iterator that never allocates: `exec` walks
/// the root's tree on every call (to find its references), so the walk
/// must cost nothing for patterns that have none.
pub(crate) enum Children<I: Iterator> {
    Many(I),
    Few([Option<I::Item>; 2]),
}

impl<I: Iterator> Iterator for Children<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<I::Item> {
        match self {
            Children::Many(iter) => iter.next(),
            Children::Few(slots) => slots.iter_mut().find_map(Option::take),
        }
    }
}

pub(crate) type Iter<'a> = Children<std::slice::Iter<'a, Pattern>>;
pub(crate) type IterMut<'a> = Children<std::slice::IterMut<'a, Pattern>>;
