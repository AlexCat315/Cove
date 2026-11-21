//! Local lexical retrieval for scoped agent tools.
//!
//! Callers split and authorize sources. The index returns item positions and
//! keeps no copy of the indexed prose. Latin text is matched by words and
//! Han, Kana and Hangul text by overlapping pairs, with single-character
//! lookup for short names and queries.

use std::collections::{BTreeSet, HashMap};

#[derive(Default)]
pub struct SearchIndex {
    terms: HashMap<Term, Vec<(usize, usize)>>,
    lengths: Vec<usize>,
    total_length: usize,
}

#[derive(Eq, Hash, Ord, PartialEq, PartialOrd)]
enum Term {
    Word(String),
    Character(char),
    Pair(char, char),
}

impl SearchIndex {
    /// Adds one searchable item and returns its position in insertion order.
    pub fn add(&mut self, title: &str, text: &str) -> usize {
        let index = self.lengths.len();
        let mut frequencies = HashMap::new();
        tokenize(text, true, |term| {
            *frequencies.entry(term).or_insert(0) += 1;
        });
        let length = frequencies.values().sum::<usize>();
        self.lengths.push(length);
        self.total_length += length;
        tokenize(title, true, |term| {
            *frequencies.entry(term).or_insert(0) += 3;
        });
        for (term, count) in frequencies {
            self.terms.entry(term).or_default().push((index, count));
        }
        index
    }

    /// Returns positions in relevance order, with insertion order resolving ties.
    pub fn search(&self, query: &str, limit: usize) -> Vec<usize> {
        if limit == 0 || self.lengths.is_empty() {
            return Vec::new();
        }
        let count = self.lengths.len() as f64;
        let average = (self.total_length as f64 / count).max(1.0);
        let mut scores = HashMap::<usize, f64>::new();
        let mut query_terms = BTreeSet::new();
        tokenize(query, false, |term| {
            query_terms.insert(term);
        });
        for term in query_terms {
            let Some(postings) = self.terms.get(&term) else {
                continue;
            };
            // BM25 with positive inverse document frequency.
            let frequency = postings.len() as f64;
            let rarity = ((count - frequency + 0.5) / (frequency + 0.5)).ln_1p();
            for &(index, occurrences) in postings {
                let occurrences = occurrences as f64;
                let normalization = 1.2 * (0.25 + 0.75 * self.lengths[index] as f64 / average);
                *scores.entry(index).or_default() +=
                    rarity * (2.2 * occurrences) / (occurrences + normalization);
            }
        }
        let mut ranked: Vec<_> = scores.into_iter().collect();
        ranked.sort_unstable_by(|(a, score_a), (b, score_b)| {
            score_b.total_cmp(score_a).then_with(|| a.cmp(b))
        });
        ranked
            .into_iter()
            .take(limit)
            .map(|(index, _)| index)
            .collect()
    }
}

fn tokenize(text: &str, index_single_characters: bool, mut emit: impl FnMut(Term)) {
    let mut word = String::new();
    let mut previous = None;
    let mut paired = false;
    for character in text.chars().chain(std::iter::once(' ')) {
        if is_east_asian(character) {
            if !word.is_empty() {
                emit(Term::Word(std::mem::take(&mut word)));
            }
            if let Some(before) = previous {
                emit(Term::Pair(before, character));
                paired = true;
            }
            if index_single_characters {
                emit(Term::Character(character));
            }
            previous = Some(character);
        } else {
            if let Some(before) = previous.take()
                && !paired
                && !index_single_characters
            {
                emit(Term::Character(before));
            }
            paired = false;
            if character.is_alphanumeric() {
                word.extend(character.to_lowercase());
            } else if !word.is_empty() {
                emit(Term::Word(std::mem::take(&mut word)));
            }
        }
    }
}

fn is_east_asian(character: char) -> bool {
    matches!(character,
        '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}'
        | '\u{20000}'..='\u{323af}' | '\u{3040}'..='\u{30ff}' | '\u{ac00}'..='\u{d7a3}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_rare_chinese_pairs_and_supports_words_without_case() {
        let mut index = SearchIndex::default();
        index.add("第一章", "沈舟走进旧城。");
        index.add("第二章", "沈舟拿出鹰纹银匣，将其交给 Aster。");
        index.add("第三章", "沈舟回到旧城。");
        assert_eq!(index.search("沈舟 鹰纹银匣", 1), vec![1]);
        assert_eq!(index.search("ASTER", 1), vec![1]);
        assert_eq!(index.search("鹰", 1), vec![1]);
        assert!(index.search("火车站", 8).is_empty());
    }
}
