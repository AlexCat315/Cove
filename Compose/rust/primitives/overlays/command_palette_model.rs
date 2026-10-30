//! Translation of `Primitives/Overlays/Command/{Search}.swift` and the search
//! half of `Primitives/Overlays/CommandPalette.swift`.
//!
//! Ranking and result projection are shared by rendering, selection and
//! scrolling, so they live here rather than in the view.
//!
//! Behaviour checklist:
//! - matching folds case, diacritics and width, and splits the query on
//!   whitespace so every term must hit
//! - scoring prefers title prefix, then word prefix, then title containment,
//!   then keywords, then a subsequence, then the group/subtitle context
//! - a term that hits none of those rejects the item
//! - enabled items outrank disabled ones; ties keep source order
//! - results group under headers only when grouping is on *and* more than one
//!   group exists
//! - recents appear once, are filtered to enabled items, and are capped
//! - arrow navigation wraps; page jumps clamp

use std::collections::HashSet;

/// Score awarded for a term that prefixes the whole title.
const SCORE_TITLE_PREFIX: i32 = 1000;
/// Score for a term that prefixes one word of the title.
const SCORE_WORD_PREFIX: i32 = 800;
/// Score for a term contained anywhere in the title.
const SCORE_TITLE_CONTAINS: i32 = 700;
/// Score for a term contained in a keyword alias.
const SCORE_ALIAS: i32 = 300;
/// Base score for a subsequence hit; gaps subtract from it.
const SCORE_SUBSEQUENCE: i32 = 200;
/// Score for a term contained in the group/subtitle context.
const SCORE_CONTEXT: i32 = 120;

/// A command that can be run from the palette.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandItem {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub group: String,
    pub keywords: Vec<String>,
    pub is_enabled: bool,
}

impl CommandItem {
    pub fn new(id: impl Into<String>, title: impl Into<String>, group: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: String::new(),
            group: group.into(),
            keywords: Vec::new(),
            is_enabled: true,
        }
    }

    pub fn with_keywords(mut self, keywords: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.keywords = keywords.into_iter().map(Into::into).collect();
        self
    }

    pub fn disabled(mut self) -> Self {
        self.is_enabled = false;
        self
    }
}

/// One row of the palette: a group header or a command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandEntry {
    Header(String),
    Item(CommandItem),
}

impl CommandEntry {
    /// The stable identity used for selection and scrolling.
    pub fn id(&self) -> String {
        match self {
            Self::Header(title) => format!("header:{title}"),
            Self::Item(item) => format!("item:{}", item.id),
        }
    }

    pub fn item(&self) -> Option<&CommandItem> {
        match self {
            Self::Header(_) => None,
            Self::Item(item) => Some(item),
        }
    }
}

/// Folds case, diacritics and width, then lowercases.
///
/// This mirrors Swift's `folding(options:caseInsensitive|diacriticInsensitive|
/// widthInsensitive)` followed by `lowercased()`, so `"Café"` and `"cafe"`
/// compare equal.
pub fn normalize(text: &str) -> String {
    text.chars()
        .map(|character| match character {
            // Fullwidth ASCII folds to its halfwidth form.
            '\u{FF01}'..='\u{FF5E}' => {
                char::from_u32(character as u32 - 0xFEE0).unwrap_or(character)
            }
            // Strip combining marks so a decomposed accent compares equal to its
            // base letter.
            '\u{0300}'..='\u{036F}' => '\0',
            // The precomposed Latin-1 supplement needs an explicit fold: `é` is
            // one character, not `e` plus a mark.
            '\u{00C0}'..='\u{00FF}' => fold_latin1(character),
            _ => character,
        })
        .filter(|character| *character != '\0')
        .collect::<String>()
        .to_lowercase()
}

/// Base letter for a precomposed Latin-1 accented character.
fn fold_latin1(character: char) -> char {
    match character {
        '\u{00C0}'..='\u{00C5}' | '\u{00E0}'..='\u{00E5}' => 'a',
        '\u{00C7}' | '\u{00E7}' => 'c',
        '\u{00C8}'..='\u{00CB}' | '\u{00E8}'..='\u{00EB}' => 'e',
        '\u{00CC}'..='\u{00CF}' | '\u{00EC}'..='\u{00EF}' => 'i',
        '\u{00D1}' | '\u{00F1}' => 'n',
        '\u{00D2}'..='\u{00D6}' | '\u{00F2}'..='\u{00F6}' | '\u{00D8}' | '\u{00F8}' => 'o',
        '\u{00D9}'..='\u{00DC}' | '\u{00F9}'..='\u{00FC}' => 'u',
        '\u{00DD}' | '\u{00FD}' | '\u{00FF}' => 'y',
        _ => character,
    }
}

/// Ranks `items` against `query`, dropping non-matches.
pub fn ranked(items: &[CommandItem], query: &str) -> Vec<CommandItem> {
    let terms: Vec<String> = normalize(query).split_whitespace().map(str::to_owned).collect();
    if terms.is_empty() {
        return items.to_vec();
    }
    let mut scored: Vec<(i32, usize, &CommandItem)> = items
        .iter()
        .enumerate()
        .filter_map(|(order, item)| score(item, &terms).map(|score| (score, order, item)))
        .collect();
    scored.sort_by(|left, right| {
        // Enabled commands always come first.
        if left.2.is_enabled != right.2.is_enabled {
            return right.2.is_enabled.cmp(&left.2.is_enabled);
        }
        // Higher score first; ties keep source order.
        if left.0 == right.0 {
            left.1.cmp(&right.1)
        } else {
            right.0.cmp(&left.0)
        }
    });
    scored.into_iter().map(|(_, _, item)| item.clone()).collect()
}

/// Scores one item, or `None` when any term fails to match.
fn score(item: &CommandItem, terms: &[String]) -> Option<i32> {
    let title = normalize(&item.title);
    let aliases: Vec<String> = item.keywords.iter().map(|keyword| normalize(keyword)).collect();
    let context = normalize(&format!("{} {}", item.subtitle, item.group));
    let words: Vec<&str> = title.split_whitespace().collect();
    let mut total = 0;
    for term in terms {
        if title.starts_with(term.as_str()) {
            total += SCORE_TITLE_PREFIX;
        } else if words.iter().any(|word| word.starts_with(term.as_str())) {
            total += SCORE_WORD_PREFIX;
        } else if title.contains(term.as_str()) {
            total += SCORE_TITLE_CONTAINS;
        } else if aliases.iter().any(|alias| alias.contains(term.as_str())) {
            total += SCORE_ALIAS;
        } else if let Some(bonus) = subsequence(&title, term) {
            total += SCORE_SUBSEQUENCE + bonus;
        } else if context.contains(term.as_str()) {
            total += SCORE_CONTEXT;
        } else {
            return None;
        }
    }
    Some(total)
}

/// Bonus for an in-order subsequence hit, penalised by the characters skipped.
///
/// Never returns less than 20, so a scattered match still counts as a weak hit
/// rather than being discarded.
fn subsequence(text: &str, term: &str) -> Option<i32> {
    let characters: Vec<char> = text.chars().collect();
    let mut cursor = 0usize;
    let mut skipped = 0i32;
    let mut previous: Option<usize> = None;
    for wanted in term.chars() {
        let found = characters[cursor..].iter().position(|value| *value == wanted)? + cursor;
        if let Some(previous) = previous {
            skipped += (found - previous) as i32 - 1;
        }
        previous = Some(found);
        cursor = found + 1;
    }
    Some((90 - skipped * 5).max(20))
}

/// Projects items into rows, optionally grouped under headers.
pub fn entries(items: &[CommandItem], groups_results: bool) -> Vec<CommandEntry> {
    let unique_groups: HashSet<&str> = items.iter().map(|item| item.group.as_str()).collect();
    if !groups_results || unique_groups.len() <= 1 {
        return items.iter().cloned().map(CommandEntry::Item).collect();
    }
    // Group order follows first appearance, so the list does not reshuffle.
    let mut order: Vec<&str> = Vec::new();
    let mut buckets: std::collections::HashMap<&str, Vec<CommandItem>> = std::collections::HashMap::new();
    for item in items {
        let group = item.group.as_str();
        if !buckets.contains_key(group) {
            order.push(group);
        }
        buckets.entry(group).or_default().push(item.clone());
    }
    let mut rows = Vec::new();
    for group in order {
        if !group.is_empty() {
            rows.push(CommandEntry::Header(group.to_owned()));
        }
        if let Some(bucket) = buckets.remove(group) {
            rows.extend(bucket.into_iter().map(CommandEntry::Item));
        }
    }
    rows
}

/// Projects items with a recents section when the query is empty.
pub fn display_entries(
    items: &[CommandItem],
    recent_ids: &[String],
    shows_recent: bool,
    limit: usize,
    groups_results: bool,
) -> Vec<CommandEntry> {
    let recent: Vec<CommandItem> = if shows_recent {
        recent_ids
            .iter()
            .filter_map(|id| {
                items
                    .iter()
                    .find(|item| &item.id == id && item.is_enabled)
                    .cloned()
            })
            .take(limit)
            .collect()
    } else {
        Vec::new()
    };
    if recent.is_empty() {
        return entries(items, groups_results);
    }
    let seen: HashSet<&str> = recent.iter().map(|item| item.id.as_str()).collect();
    let remaining: Vec<CommandItem> = items
        .iter()
        .filter(|item| !seen.contains(item.id.as_str()))
        .cloned()
        .collect();
    let mut rows = vec![CommandEntry::Header("Recent".to_owned())];
    rows.extend(recent.into_iter().map(CommandEntry::Item));
    rows.extend(entries(&remaining, groups_results));
    rows
}

/// The next highlight index.
///
/// Arrow keys wrap; page jumps clamp. The modulus is reduced before adding so a
/// huge delta cannot overflow.
pub fn destination(count: usize, current: Option<usize>, delta: i64, wraps: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let count_i = count as i64;
    let Some(current) = current.filter(|index| *index < count) else {
        // No valid current: a backward move starts at the end, forward at the start.
        return Some(if delta < 0 { count - 1 } else { 0 });
    };
    let current_i = current as i64;
    if wraps {
        let step = delta % count_i;
        if step >= 0 {
            return Some(if current_i >= count_i - step {
                (current_i - (count_i - step)) as usize
            } else {
                (current_i + step) as usize
            });
        }
        return Some(if current_i < -step {
            (count_i - (-step - current_i)) as usize
        } else {
            (current_i + step) as usize
        });
    }
    if delta >= 0 {
        Some((current_i + delta.min(count_i - 1 - current_i)) as usize)
    } else {
        Some((current_i + delta.max(-current_i)) as usize)
    }
}

/// Records a command as recently used, moving it to the front and capping the
/// list.
pub fn record_recent(recent_ids: &mut Vec<String>, id: &str, limit: usize) {
    if limit == 0 {
        return;
    }
    recent_ids.retain(|existing| existing != id);
    recent_ids.insert(0, id.to_owned());
    if recent_ids.len() > limit {
        recent_ids.truncate(limit);
    }
}
