//! Questions about the pattern at a place in a root's tree (DESIGN §22).
//!
//! v12 hands out live child patterns that know their parents. Rust patterns
//! have no parents (DESIGN §2), so `js/pattern.mjs` names a child by its
//! root and a path (`Pattern::at_path` numbering, read through
//! `Introspection::at` so a reference that hasn't parsed is followed to
//! what it would resolve to), and every read below takes that path. A
//! pattern built in JS is its own root, at the empty path.

use std::cell::RefCell;

use cpat_core::{Handle, Introspection, Pattern, PatternPath};
use wasm_bindgen::prelude::*;

use crate::JsPattern;
use crate::host::PatternPredicate;

/// Paths as one flat array for JS: each path's length, then its steps.
fn flat_paths<'h>(handles: impl IntoIterator<Item = Handle<'h>>) -> Vec<u32> {
    let mut flat = Vec::new();
    for handle in handles {
        let steps = handle.path.steps();
        flat.push(steps.len() as u32);
        flat.extend_from_slice(steps);
    }
    flat
}

impl JsPattern {
    fn read<R>(&self, path: &[u32], read: impl FnOnce(&Handle<'_>) -> R) -> Option<R> {
        Introspection::new(&self.inner)
            .at(&PatternPath::new(path))
            .map(|handle| read(&handle))
    }

    /// The pattern at `path` as a root of its own: the root itself, or a
    /// clone named as the instance there answers (a reference's target
    /// after the reference).
    pub(crate) fn owned_at(&self, path: &[u32]) -> Option<Pattern> {
        if path.is_empty() {
            return Some(self.inner.clone());
        }
        self.read(path, |handle| handle.pattern.clone_named(handle.name()))
    }
}

#[wasm_bindgen(js_class = Pattern)]
impl JsPattern {
    /// The name the instance at `path` answers to, or `undefined` when the
    /// path leads nowhere.
    #[wasm_bindgen(js_name = nameAt)]
    pub fn name_at(&self, path: Vec<u32>) -> Option<String> {
        self.read(&path, |handle| handle.name().to_string())
    }

    /// v12's `type` of the pattern at `path`.
    #[wasm_bindgen(js_name = typeAt)]
    pub fn type_at(&self, path: Vec<u32>) -> Option<String> {
        self.read(&path, |handle| handle.pattern.type_name().to_string())
    }

    /// The id the instance at `path` shares with its clones. A choice's
    /// operator has none of its own (v12 mints one per build); it answers
    /// with its place.
    #[wasm_bindgen(js_name = idAt)]
    pub fn id_at(&self, path: Vec<u32>) -> Option<String> {
        self.read(&path, |handle| match handle.id() {
            Some(id) => id.to_string(),
            None => format!("operator:{path:?}"),
        })
    }

    /// How many children the instance at `path` has.
    #[wasm_bindgen(js_name = childCountAt)]
    pub fn child_count_at(&self, path: Vec<u32>) -> u32 {
        let introspection = Introspection::new(&self.inner);
        let mut child = path.clone();
        child.push(0);
        let mut count = 0;
        while introspection.at(&PatternPath::new(&child)).is_some() {
            count += 1;
            *child.last_mut().expect("pushed above") = count;
        }
        count
    }

    /// A literal's token, or `undefined` for any other pattern.
    #[wasm_bindgen(js_name = tokenAt)]
    pub fn token_at(&self, path: Vec<u32>) -> Option<String> {
        self.read(&path, |handle| match handle.pattern {
            Pattern::Literal(literal) => Some(literal.token().to_string()),
            _ => None,
        })
        .flatten()
    }

    /// A regex's source, or `undefined` for any other pattern.
    #[wasm_bindgen(js_name = regexAt)]
    pub fn regex_at(&self, path: Vec<u32>) -> Option<String> {
        self.read(&path, |handle| match handle.pattern {
            Pattern::Regex(regex) => Some(regex.source().to_string()),
            _ => None,
        })
        .flatten()
    }

    /// v12 `clone(name?)` of the pattern at `path`: a new root.
    #[wasm_bindgen(js_name = cloneAt)]
    pub fn clone_at(&self, path: Vec<u32>, name: Option<String>) -> Option<JsPattern> {
        let pattern = self.owned_at(&path)?;
        let name = name.unwrap_or_else(|| pattern.name().to_string());
        Some(pattern.clone_named(&name).into())
    }

    /// v12 `isEqual` between the pattern at `path` and `other`'s at
    /// `other_path`.
    #[wasm_bindgen(js_name = isEqualAt)]
    pub fn is_equal_at(&self, path: Vec<u32>, other: &JsPattern, other_path: Vec<u32>) -> bool {
        let mine = Introspection::new(&self.inner);
        let theirs = Introspection::new(&other.inner);
        match (
            mine.at(&PatternPath::new(&path)),
            theirs.at(&PatternPath::new(&other_path)),
        ) {
            (Some(a), Some(b)) => a.pattern.is_equal(b.pattern),
            _ => false,
        }
    }

    /// v12 `getTokens`.
    #[wasm_bindgen(js_name = tokensAt)]
    pub fn tokens_at(&self, path: Vec<u32>) -> Vec<String> {
        Introspection::new(&self.inner).tokens(&PatternPath::new(&path))
    }

    /// v12 `getTokensAfter(child)`, the child by index.
    #[wasm_bindgen(js_name = tokensAfterAt)]
    pub fn tokens_after_at(&self, path: Vec<u32>, child: u32) -> Vec<String> {
        Introspection::new(&self.inner).tokens_after(&PatternPath::new(&path), child as usize)
    }

    /// v12 `getNextTokens`.
    #[wasm_bindgen(js_name = nextTokensAt)]
    pub fn next_tokens_at(&self, path: Vec<u32>) -> Vec<String> {
        Introspection::new(&self.inner).next_tokens(&PatternPath::new(&path))
    }

    /// v12 `getPatterns`, as flat paths (see `flat_paths`).
    #[wasm_bindgen(js_name = patternsAt)]
    pub fn patterns_at(&self, path: Vec<u32>) -> Vec<u32> {
        flat_paths(Introspection::new(&self.inner).patterns(&PatternPath::new(&path)))
    }

    /// v12 `getPatternsAfter(child)`, as flat paths.
    #[wasm_bindgen(js_name = patternsAfterAt)]
    pub fn patterns_after_at(&self, path: Vec<u32>, child: u32) -> Vec<u32> {
        flat_paths(
            Introspection::new(&self.inner)
                .patterns_after(&PatternPath::new(&path), child as usize),
        )
    }

    /// v12 `getNextPatterns`, as flat paths.
    #[wasm_bindgen(js_name = nextPatternsAt)]
    pub fn next_patterns_at(&self, path: Vec<u32>) -> Vec<u32> {
        flat_paths(Introspection::new(&self.inner).next_patterns(&PatternPath::new(&path)))
    }

    /// v12 `find(predicate)` from the pattern at `path`: the path of the
    /// first pattern `predicate` accepts, or `undefined`. The predicate is
    /// shown each candidate as a pattern of its own (a clone). A throw from
    /// it stops the search and is thrown again here, once wasm has let go
    /// of everything it borrowed.
    #[wasm_bindgen(js_name = findAt)]
    pub fn find_at(
        &self,
        path: Vec<u32>,
        predicate: &PatternPredicate,
    ) -> Result<Option<Vec<u32>>, JsValue> {
        let Some(place) = self.owned_at(&path) else {
            return Ok(None);
        };
        let thrown = RefCell::new(None);
        let found = place.find(|candidate| {
            if thrown.borrow().is_some() {
                return false;
            }
            match predicate.test(candidate.clone().into()) {
                Ok(accepted) => accepted,
                Err(error) => {
                    *thrown.borrow_mut() = Some(error);
                    false
                }
            }
        });
        let found = found.map(|handle| {
            let mut steps = path.clone();
            steps.extend_from_slice(handle.path.steps());
            steps
        });
        match thrown.into_inner() {
            Some(error) => Err(error),
            None => Ok(found),
        }
    }

    /// A context's definitions (v12 `getPatternsWithinContext`), each a
    /// clone named as the context knows it, by name. Empty for anything
    /// else.
    #[wasm_bindgen(js_name = patternsWithinContextAt)]
    pub fn patterns_within_context_at(&self, path: Vec<u32>) -> Vec<JsPattern> {
        self.read(&path, |handle| match handle.pattern {
            Pattern::Context(context) => {
                let mut named: Vec<_> = context.patterns_within_context().into_iter().collect();
                named.sort_by_key(|(name, _)| *name);
                named
                    .into_iter()
                    .map(|(name, pattern)| pattern.clone_named(name).into())
                    .collect()
            }
            _ => Vec::new(),
        })
        .unwrap_or_default()
    }

    /// v12 `setTokens` on this root: the tokens a regex, take-until or
    /// custom pattern offers to autocomplete. Throws for any other kind.
    #[wasm_bindgen(js_name = setTokens)]
    pub fn set_tokens(&mut self, tokens: Vec<String>) -> Result<(), JsError> {
        let name = self.inner.name();
        let kind = self.inner.type_name();
        match self.inner.tokenizable_mut() {
            Some(target) => {
                target.set_tokens(tokens);
                Ok(())
            }
            None => Err(JsError::new(&format!(
                "`{name}` (a {kind}) can't take tokens; only a regex, a take-until or a custom pattern can"
            ))),
        }
    }
}
