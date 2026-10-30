//! Persistent run tree mapping logical lines to visual rows. Unmeasured lines
//! start at one row; measuring a wrapped line replaces only its search path.
//! Text edits splice the affected lines, retaining measurements on both sides.

use std::rc::Rc;

#[derive(Clone)]
enum VisualLineStorage {
    Run {
        rows_per_line: usize,
    },
    Branch {
        left: Rc<VisualLineNode>,
        right: Rc<VisualLineNode>,
    },
}

#[derive(Clone)]
pub struct VisualLineNode {
    storage: VisualLineStorage,
    pub lines: usize,
    pub rows: usize,
    pub height: usize,
}

thread_local! {
    static EMPTY: Rc<VisualLineNode> = Rc::new(VisualLineNode::run(0, 1));
}

fn empty_node() -> Rc<VisualLineNode> {
    EMPTY.with(|node| node.clone())
}

impl VisualLineNode {
    fn run(lines: usize, rows_per_line: usize) -> Self {
        Self {
            storage: VisualLineStorage::Run { rows_per_line },
            lines,
            rows: lines * rows_per_line,
            height: if lines == 0 { 0 } else { 1 },
        }
    }

    fn branch(left: Rc<VisualLineNode>, right: Rc<VisualLineNode>) -> Self {
        Self {
            lines: left.lines + right.lines,
            rows: left.rows + right.rows,
            height: left.height.max(right.height) + 1,
            storage: VisualLineStorage::Branch { left, right },
        }
    }

    pub fn rows_before_line(&self, index: usize) -> usize {
        match &self.storage {
            VisualLineStorage::Run { rows_per_line } => index * rows_per_line,
            VisualLineStorage::Branch { left, right } => {
                if index <= left.lines {
                    left.rows_before_line(index)
                } else {
                    left.rows + right.rows_before_line(index - left.lines)
                }
            }
        }
    }

    pub fn location_at_row(&self, row: usize) -> (usize, usize) {
        match &self.storage {
            VisualLineStorage::Run { rows_per_line } => {
                if *rows_per_line == 0 {
                    // Hidden runs contain no rows; callers never address them.
                    return (0, 0);
                }
                (row / rows_per_line, row % rows_per_line)
            }
            VisualLineStorage::Branch { left, right } => {
                if row < left.rows {
                    return left.location_at_row(row);
                }
                let (line, subrow) = right.location_at_row(row - left.rows);
                (left.lines + line, subrow)
            }
        }
    }
}

pub fn join(left: Rc<VisualLineNode>, right: Rc<VisualLineNode>) -> Rc<VisualLineNode> {
    if left.lines == 0 {
        return right;
    }
    if right.lines == 0 {
        return left;
    }
    if let (
        VisualLineStorage::Run { rows_per_line: a },
        VisualLineStorage::Run { rows_per_line: b },
    ) = (&left.storage, &right.storage)
    {
        if a == b {
            return Rc::new(VisualLineNode::run(left.lines + right.lines, *a));
        }
    }
    if left.height > right.height + 1 {
        if let VisualLineStorage::Branch { left: a, right: b } = &left.storage {
            return balanced(a.clone(), join(b.clone(), right));
        }
    }
    if right.height > left.height + 1 {
        if let VisualLineStorage::Branch { left: a, right: b } = &right.storage {
            return balanced(join(left, a.clone()), b.clone());
        }
    }
    Rc::new(VisualLineNode::branch(left, right))
}

fn branch_parts(node: &VisualLineNode) -> Option<(Rc<VisualLineNode>, Rc<VisualLineNode>)> {
    match &node.storage {
        VisualLineStorage::Branch { left, right } => Some((left.clone(), right.clone())),
        VisualLineStorage::Run { .. } => None,
    }
}

fn balanced(left: Rc<VisualLineNode>, right: Rc<VisualLineNode>) -> Rc<VisualLineNode> {
    if left.height > right.height + 1 {
        if let Some((a, b)) = branch_parts(&left) {
            if a.height >= b.height {
                return Rc::new(VisualLineNode::branch(
                    a,
                    Rc::new(VisualLineNode::branch(b, right)),
                ));
            }
            if let Some((c, d)) = branch_parts(&b) {
                return Rc::new(VisualLineNode::branch(
                    Rc::new(VisualLineNode::branch(a, c)),
                    Rc::new(VisualLineNode::branch(d, right)),
                ));
            }
        }
    }
    if right.height > left.height + 1 {
        if let Some((a, b)) = branch_parts(&right) {
            if b.height >= a.height {
                return Rc::new(VisualLineNode::branch(
                    Rc::new(VisualLineNode::branch(left, a)),
                    b,
                ));
            }
            if let Some((c, d)) = branch_parts(&a) {
                return Rc::new(VisualLineNode::branch(
                    Rc::new(VisualLineNode::branch(left, c)),
                    Rc::new(VisualLineNode::branch(d, b)),
                ));
            }
        }
    }
    Rc::new(VisualLineNode::branch(left, right))
}

fn split_handle(
    node: Rc<VisualLineNode>,
    at_line: usize,
) -> (Rc<VisualLineNode>, Rc<VisualLineNode>) {
    if at_line == 0 {
        return (empty_node(), node);
    }
    if at_line >= node.lines {
        return (node, empty_node());
    }
    match &node.storage {
        VisualLineStorage::Run { rows_per_line } => (
            Rc::new(VisualLineNode::run(at_line, *rows_per_line)),
            Rc::new(VisualLineNode::run(node.lines - at_line, *rows_per_line)),
        ),
        VisualLineStorage::Branch { left, right } => {
            if at_line < left.lines {
                let (first, rest) = split_handle(left.clone(), at_line);
                (first, join(rest, right.clone()))
            } else {
                let (first, rest) = split_handle(right.clone(), at_line - left.lines);
                (join(left.clone(), first), rest)
            }
        }
    }
}

/// Snapshot semantics: a clone shares the current root, and later edits build
/// new nodes above it, so an old handle keeps reporting its own totals.
#[derive(Clone)]
pub struct TextVisualLineIndex {
    root: Rc<VisualLineNode>,
}

impl TextVisualLineIndex {
    pub fn new(line_count: usize) -> Self {
        Self {
            root: Rc::new(VisualLineNode::run(line_count.max(1), 1)),
        }
    }

    pub fn line_count(&self) -> usize {
        self.root.lines
    }

    pub fn row_count(&self) -> usize {
        self.root.rows
    }

    pub fn height(&self) -> usize {
        self.root.height
    }

    pub fn first_row_of_line(&self, index: usize) -> usize {
        self.root.rows_before_line(index.min(self.line_count()))
    }

    pub fn location_at_row(&self, row: usize) -> (usize, usize) {
        self.root
            .location_at_row(row.min(self.row_count().saturating_sub(1)))
    }

    pub fn measure(&mut self, line: usize, rows: usize) {
        if line >= self.line_count() {
            return;
        }
        let count = rows.max(1);
        if self.first_row_of_line(line + 1) - self.first_row_of_line(line) == count {
            return;
        }
        self.replace_with_rows(line..(line + 1), 1, count);
    }

    pub fn hide(&mut self, range: std::ops::Range<usize>) {
        self.replace_with_rows(range.clone(), range.end - range.start, 0);
    }

    pub fn replace(&mut self, range: std::ops::Range<usize>, count: usize) {
        self.replace_with_rows(range, count.max(1), 1);
    }

    fn replace_with_rows(
        &mut self,
        range: std::ops::Range<usize>,
        count: usize,
        rows_per_line: usize,
    ) {
        let lower = self.line_count().min(range.start);
        let upper = self.line_count().min(lower.max(range.end));
        let (before, rest) = split_handle(self.root.clone(), lower);
        let (_, after) = split_handle(rest, upper - lower);
        self.root = join(
            join(before, Rc::new(VisualLineNode::run(count, rows_per_line))),
            after,
        );
    }
}
