//! Stories for primitives the migration audit listed as having no gallery
//! coverage (`Docs/audit-swift-migration-dead-code-and-gallery-coverage.md` §5.4).
//!
//! These are not decorative pages: each one exercises the primitive's real
//! public surface so the gallery doubles as a smoke test for it.
//!
//! - `SingleLineText` is a pure text-fitting helper, so the page shows the
//!   function's output at several widths instead of pretending it is a view.
//! - `CompletionPopover` and `RouteOutlet` are views and are rendered for real.

use cove_composition::{PrimitiveView, Scope};
use cove_core::Rect;
use cove_core::State;
use cove_core::scene::{Align, FlexDirection, Justify, LayoutNode, Node, NodeResource, Wrap};
use cove_ui_compose::impl_primitive_view;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::{Button, CompletionPopover, TextCompletionItem};
use cove_ui_compose::primitives::feedback::ProgressValue;
use cove_ui_compose::primitives::navigation::RouteOutlet;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::single_line::{SingleLineText, normalized_single_line};
use cove_ui_compose::theme::theme_for_node;
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};
use std::rc::Rc;

use super::feedback::StorySection;
use crate::gallery_content::catalog::{CowSize, GalleryOptions};

fn column(items: Vec<AnyView>, spacing: f32) -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        spacing,
        AnyView::new(TupleView::new(items)),
    ))
}

/// `SingleLineText`: how a single line is measured and truncated to fit.
pub struct SingleLineTextStory {
    options: GalleryOptions,
}

impl SingleLineTextStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self { options }
    }

    /// The sample sentence the page fits into each width.
    pub const SOURCE: &'static str = "CoveUI keeps every label on one line and truncates the tail";

    /// Approximate advance per character for the page's current density. The
    /// gallery's density toggle changes this, so the truncation points move.
    pub fn char_width(size: CowSize) -> f32 {
        match size {
            CowSize::Mini => 6.0,
            CowSize::Small => 6.5,
            CowSize::Regular => 7.0,
            CowSize::Large => 8.5,
        }
    }

    /// `fitting` at `width`, measured at `char_width` per character.
    pub fn fitted(width: f32, char_width: f32) -> String {
        SingleLineText::fitting(Self::SOURCE, width, |text| {
            text.chars().count() as f32 * char_width
        })
    }
}

impl View for SingleLineTextStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let char_width = Self::char_width(self.options.size);
        let widths = [420.0_f32, 280.0, 180.0, 110.0];
        let rows = widths
            .iter()
            .map(|width| {
                AnyView::new(Text::new(format!(
                    "{width:>5.0}pt @ {char_width:.1}pt/char → {}",
                    Self::fitted(*width, char_width)
                )))
            })
            .collect::<Vec<_>>();

        let normalized = normalized_single_line("first line\nsecond line\r\nthird line");
        let notes = column(
            vec![
                AnyView::new(Text::new(format!("normalized: {normalized}"))),
                AnyView::new(Text::new(
                    "`fitting` only truncates when the measured width exceeds the budget; \
                     `normalized_single_line` collapses every line break into one space.",
                )),
            ],
            8.0,
        );

        AnyView::new(column(
            vec![
                AnyView::new(StorySection::new(
                    "Fitted widths",
                    "The same sentence measured at four widths. Narrower budgets truncate the tail.",
                    column(rows, 6.0),
                )),
                AnyView::new(StorySection::new(
                    "Line breaks",
                    "Line breaks are folded before fitting, so a multi-line string still renders on one line.",
                    notes,
                )),
            ],
            20.0,
        ))
    }
}

/// `CompletionPopover`: the nonmodal completion list, driven by buttons.
pub struct CompletionPopoverStory {
    options: GalleryOptions,
    selected: State<usize>,
    accepted: State<String>,
    input: State<cove_ui_compose::primitives::text::text_buffer::TextBuffer>,
}

impl CompletionPopoverStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            selected: State::new(0),
            accepted: State::new(String::new()),
            input: State::new(cove_ui_compose::primitives::text::text_buffer::TextBuffer::empty()),
        }
    }

    /// The rows the popover lists.
    pub fn items() -> Vec<TextCompletionItem> {
        [
            (
                "render",
                "render(_ view: View)",
                "Render a view into the current surface.",
            ),
            (
                "layout",
                "layout(width: Float)",
                "Recompute layout for the available width.",
            ),
            (
                "measure",
                "measure(in: Size)",
                "Measure the content without changing its placement.",
            ),
            (
                "focus",
                "focus(_ target: Node)",
                "Move keyboard focus to a control.",
            ),
            (
                "reveal",
                "reveal(_ range: Range)",
                "Scroll the requested range into view.",
            ),
            (
                "animate",
                "animate(_:to:)",
                "Animate a value using the active motion settings.",
            ),
            (
                "print",
                "print(_ value: Any)",
                "Write a value to the debug console.",
            ),
            (
                "private",
                "Access modifier",
                "Limit a declaration to its enclosing scope.",
            ),
            (
                "padding",
                "padding(_ value: Float)",
                "Inset content on all sides.",
            ),
            (
                "position",
                "position(x:y:)",
                "Position a view within its parent.",
            ),
            (
                "background",
                "background(_ color: Color)",
                "Fill the area behind the content.",
            ),
            (
                "border",
                "border(_ color: Color)",
                "Draw a border around a view.",
            ),
            ("clip", "clip(to: Rect)", "Clip rendering to a rectangle."),
            ("color", "Color", "Create a color value."),
            (
                "frame",
                "frame(width:height:)",
                "Constrain the content dimensions.",
            ),
            (
                "font",
                "font(_ font: Font)",
                "Set the font for text descendants.",
            ),
            (
                "opacity",
                "opacity(_ value: Float)",
                "Set the content opacity between zero and one.",
            ),
            ("offset", "offset(x:y:)", "Translate the rendered content."),
            ("scroll", "scroll(to: Point)", "Scroll to an offset."),
            (
                "selection",
                "selection: Range",
                "Read the current selected range.",
            ),
        ]
        .into_iter()
        .map(|(label, detail, documentation)| {
            let mut item = TextCompletionItem::new(
                label,
                label,
                if label == "print" {
                    "print(value)"
                } else {
                    label
                },
            );
            item.detail = Some(detail.into());
            item.documentation = Some(documentation.into());
            item
        })
        .collect()
    }

    pub fn selected_index(&self) -> usize {
        self.selected.get()
    }

    pub fn accepted_label(&self) -> String {
        self.accepted.get()
    }
}

impl View for CompletionPopoverStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        use super::showcase::{bounded, note, row, stack};
        use cove_ui_compose::primitives::text::text_field::{
            TextField, TextFieldBehavior, TextFieldCodeEditing,
        };
        let enabled = self.options.is_enabled;
        let accepted = self.accepted.clone();
        let code = TextFieldCodeEditing {
            on_request_completion: Some(Rc::new(|request, reply| {
                let prefix = request.prefix.to_lowercase();
                reply(
                    Self::items()
                        .into_iter()
                        .filter(|item| item.label.starts_with(&prefix))
                        .collect(),
                );
            })),
            on_accept_completion: Some(Rc::new(move |item| accepted.set(item.label))),
            ..TextFieldCodeEditing::default()
        };
        let input = TextField::new("Type a symbol", self.input.binding())
            .code_editing(code)
            .behavior(TextFieldBehavior {
                disabled: !enabled,
                ..TextFieldBehavior::default()
            })
            .debug_name("completion-input");
        let accepted = self.accepted.clone();
        let text = self.input.clone();
        let panel = CompletionPopover::new(Self::items(), self.selected.get())
            .enabled(enabled)
            .on_accept(move |item| {
                text.set(
                    cove_ui_compose::primitives::text::text_buffer::TextBuffer::new(
                        &item.insert_text,
                    ),
                );
                accepted.set(item.label);
            });
        let previous = self.selected.clone();
        let next = self.selected.clone();
        let last = self.selected.clone();
        stack(vec![
            AnyView::new(StorySection::new(
                "Live completion",
                "Type a prefix or press Control-Space. Arrows navigate, Return inserts, Escape dismisses.",
                stack(vec![
                    bounded(AnyView::new(input), 480.0),
                    note(if self.accepted.get().is_empty() {
                        "No completion accepted".into()
                    } else {
                        format!("Inserted: {}", self.accepted.get())
                    }),
                ]),
            )),
            AnyView::new(StorySection::new(
                "Candidate browser",
                "Twenty symbols, ten visible rows, with details and documentation for the highlight.",
                stack(vec![
                    bounded(
                        AnyView::new(panel).debug_name("completion-candidates"),
                        580.0,
                    ),
                    row(vec![
                        AnyView::new(
                            Button::title("Previous", move || {
                                previous.set(previous.get().saturating_sub(1))
                            })
                            .enabled(enabled && self.selected.get() > 0),
                        ),
                        AnyView::new(
                            Button::title("Next", move || {
                                next.set((next.get() + 1).min(Self::items().len() - 1))
                            })
                            .enabled(enabled && self.selected.get() + 1 < Self::items().len()),
                        ),
                        AnyView::new(
                            Button::title("Last candidate", move || {
                                last.set(Self::items().len() - 1)
                            })
                            .enabled(enabled),
                        ),
                        note(format!(
                            "{} / {}",
                            self.selected.get() + 1,
                            Self::items().len()
                        )),
                    ]),
                ]),
            )),
        ])
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; selected, accepted, input)
    }
}

/// The destinations the route outlet can render.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DemoRoute {
    Overview,
    Detail,
    Settings,
}

impl DemoRoute {
    pub const ALL: [DemoRoute; 3] = [DemoRoute::Overview, DemoRoute::Detail, DemoRoute::Settings];

    pub fn title(self) -> &'static str {
        match self {
            DemoRoute::Overview => "Overview",
            DemoRoute::Detail => "Detail",
            DemoRoute::Settings => "Settings",
        }
    }
}

/// `RouteOutlet`: a composition boundary that swaps content per selected route.
pub struct RouteOutletStory {
    options: GalleryOptions,
    route: State<DemoRoute>,
}

impl RouteOutletStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            route: State::new(DemoRoute::Overview),
        }
    }

    pub fn route(&self) -> DemoRoute {
        self.route.get()
    }
}

impl View for RouteOutletStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let outlet = RouteOutlet::new(self.route.binding(), |route: DemoRoute| {
            AnyView::new(Text::new(format!(
                "{} content. The outlet rebuilt this subtree with a new identity.",
                route.title()
            )))
        });

        let is_enabled = self.options.is_enabled;
        let buttons = DemoRoute::ALL
            .iter()
            .map(|route| {
                let selection = self.route.clone();
                let target = *route;
                AnyView::new(
                    Button::title(route.title(), move || selection.set(target)).enabled(is_enabled),
                )
            })
            .collect::<Vec<_>>();

        AnyView::new(column(
            vec![
                AnyView::new(StorySection::new(
                    "Route selection",
                    "Each button writes the binding; the outlet renders only the selected destination.",
                    AnyView::new(BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::FlexStart,
                        Wrap::Wrap,
                        8.0,
                        AnyView::new(TupleView::new(buttons)),
                    )),
                )),
                AnyView::new(StorySection::new(
                    "Destination",
                    "The identity is the route value, so switching routes does not reuse the old subtree.",
                    AnyView::new(outlet),
                )),
            ],
            20.0,
        ))
    }
}

/// A bar painted from the node's `ProgressValue` resource instead of from the
/// view's own state, so the page exercises the resource's animation contract.
struct ProgressValueBar {
    value: State<f64>,
    determinate: bool,
    height: f32,
}

impl PrimitiveView for ProgressValueBar {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        let resource: Rc<dyn NodeResource> = ProgressValue::new();
        node.add_resource(resource);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        if let Some(layout) = node.layout_node() {
            layout.set_height(Some(self.height));
        }
        if let Some(presentation) = node.first_resource::<ProgressValue>() {
            presentation.update(node, self.value.get().clamp(0.0, 1.0), self.determinate);
        }
        let owner = node.downgrade();
        let presentation = node.first_resource::<ProgressValue>();
        let determinate = self.determinate;
        node.update_draw(determinate, move |list, origin| {
            let Some(owner) = Node::upgrade(&owner) else {
                return;
            };
            let frame = owner.frame();
            if frame.width <= 1.0 || frame.height <= 1.0 {
                return;
            }
            let theme = theme_for_node(&owner);
            let radius = frame.height / 2.0;
            let _ = list.add_rounded_rect(
                Rect::new(origin.x, origin.y, frame.width, frame.height),
                radius,
                theme.colors.surface_variant,
            );
            let Some(presentation) = presentation.as_ref() else {
                return;
            };
            let value = presentation.value().clamp(0.0, 1.0) as f32;
            if value <= 0.0 {
                return;
            }
            let width = (frame.width * value).max(frame.height);
            let _ = list.add_rounded_rect(
                Rect::new(origin.x, origin.y, width, frame.height),
                radius,
                theme.colors.accent,
            );
        });
    }
}
impl_primitive_view!(ProgressValueBar);

/// `ProgressValue`: the shared animated value behind the progress controls.
pub struct ProgressValueStory {
    options: GalleryOptions,
    value: State<f64>,
    determinate: State<bool>,
}

impl ProgressValueStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            value: State::new(0.35),
            determinate: State::new(true),
        }
    }

    /// The steps the page jumps to.
    pub const STEPS: [f64; 4] = [0.0, 0.35, 0.7, 1.0];

    pub fn value(&self) -> f64 {
        self.value.get()
    }

    pub fn is_determinate(&self) -> bool {
        self.determinate.get()
    }
}

impl View for ProgressValueStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let bar = AnyView::new(
            ProgressValueBar {
                value: self.value.clone(),
                determinate: self.determinate.get(),
                height: 14.0,
            }
            .frame_width(320.0),
        );

        let steps = Self::STEPS
            .iter()
            .map(|step| {
                let value = self.value.clone();
                let target = *step;
                AnyView::new(
                    Button::title(format!("{:.0}%", step * 100.0), move || value.set(target))
                        .enabled(self.options.is_enabled),
                )
            })
            .collect::<Vec<_>>();

        let toggle = {
            let determinate = self.determinate.clone();
            move || determinate.set(!determinate.get())
        };
        let mut controls = steps;
        controls.push(AnyView::new(
            Button::title(
                if self.determinate.get() {
                    "Determinate: on"
                } else {
                    "Determinate: off (snaps)"
                },
                toggle,
            )
            .enabled(self.options.is_enabled),
        ));

        let readout = Text::new(format!(
            "target {:.0}% · determinate: {}",
            self.value.get() * 100.0,
            self.determinate.get()
        ));

        AnyView::new(column(
            vec![
                AnyView::new(StorySection::new(
                    "Value state",
                    "The bar reads its fill from the node's `ProgressValue`, so a value change repaints without rebuilding layout.",
                    column(
                        vec![
                            bar,
                            AnyView::new(BoxView::new(
                                FlexDirection::Row,
                                Align::Center,
                                Justify::FlexStart,
                                Wrap::Wrap,
                                8.0,
                                AnyView::new(TupleView::new(controls)),
                            )),
                            AnyView::new(readout),
                        ],
                        12.0,
                    ),
                )),
                AnyView::new(StorySection::new(
                    "Contract",
                    "`update(node, target, determinate)` interpolates through the theme's medium motion when determinate, and snaps when it is not — or when reduced motion is on. Switching to indeterminate and back snaps to each new value, which is what the toggle shows.",
                    column(
                        vec![AnyView::new(Text::new(
                            "`ProgressView` and `ProgressCircle` both install this resource; this page drives it directly so the animation is visible on a plain bar.",
                        ))],
                        8.0,
                    ),
                )),
            ],
            20.0,
        ))
    }
}
