use super::*;

/// the popup floats clear of the field it belongs to rather than sitting on it,
/// so the field stays readable while the list is open.
const POPUP_GAP: f32 = 8.0;
/// the list grows out of the field rather than appearing over it, so it opens
/// by rising the last few pixels into place.
const POPUP_RISE: f32 = 10.0;
/// the friends button and its gap are not part of the field, so the popup stops
/// short of them and lines up with the text you are typing.
const POPUP_RIGHT_INSET: f32 = 64.0;
/// the list does not stretch to the field. anchored to the left edge and capped,
/// a name and its count stay inside one eye-span; a full-width popup on an
/// ultrawide transcript would put a couple of thousand pixels between them.
const POPUP_MAX_WIDTH: f32 = 480.0;
const POPUP_PADDING: f32 = 6.0;
const ROW_HEIGHT: f32 = 38.0;
/// a person carries a face and a reason, so their row is a little taller than
/// a bare channel name.
const PERSON_ROW_HEIGHT: f32 = 40.0;
const PORTRAIT_FRAME: f32 = 24.0;
const PORTRAIT_FACE: f32 = 20.0;
const DOT_SIZE: f32 = 9.0;
/// somebody you cannot reach right now is still worth offering — a whisper
/// waits for them — and so is a channel with nobody in it, since joining is how
/// it stops being empty. both dim rather than leaving the list.
const ABSENT_OPACITY: f32 = 0.55;
/// `rgba` reads `0xRRGGBBAA`, so a dot colour has to be shifted up before it
/// can carry an alpha.
const DOT_GLOW_ALPHA: u32 = 0xcc;
const CREATE_HEIGHT: f32 = 30.0;
const FOOTER_HEIGHT: f32 = 26.0;
/// the frame's own edge. its blue rule sits 4px in, so the content clears it by
/// one more pixel rather than painting a row fill over it.
const FRAME_INSET: f32 = 5.0;
/// rows, the create action, and the footer share one gutter, so the popup reads
/// as a single column. measured from inside the frame, so the gutter reads the
/// same 12px it does in a modal.
const ROW_INSET: f32 = 12.0 - FRAME_INSET;
/// counts sit in a fixed column so the numbers stack cleanly whatever their
/// width.
const COUNT_WIDTH: f32 = 44.0;
const ICON_SIZE: f32 = 22.0;
/// the rail that marks the selected row, in place of the border a modal row
/// wears — a full box here would fight the popup's own edge.
const RAIL_WIDTH: f32 = 2.0;

const CREATE: u32 = 0x00f0_aa64;
const KEY: u32 = 0x00a9_b8cc;

impl ComposerComponent {
    /// the result list that hangs above the composer while a command is being
    /// typed. it is anchored to the field rather than centred like the modal,
    /// because it is an extension of what you are typing.
    pub(super) fn command_popup(
        &self,
        results: &CommandResults,
        closing: bool,
        assets: &UiAssets,
        cx: &mut Context<SuperiorityView>,
    ) -> AnyElement {
        let selection = results.selection(self.command_selected);
        // the list wears the colour of the trigger that opened it, so `/w`
        // reads as the same thread of purple from the command word through to
        // the conversation it lands in
        let tint = results.kind.tint();
        let mut list = div().flex().flex_col().py(px(POPUP_PADDING));
        for (index, row) in results.rows.iter().enumerate() {
            let selected = index == selection;
            list = list.child(match row {
                CommandRow::Channel(row) => {
                    channel_row(index, row, &results.query, selected, tint, cx)
                }
                CommandRow::Person(row) => {
                    person_row(index, row, results, selected, tint, assets, cx)
                }
            });
        }
        if results.create.is_some() {
            list = list.child(create_row(
                &results.query,
                results.rows.len(),
                selection == results.rows.len(),
                tint,
                cx,
            ));
        }

        let popup = div()
            .id("join-command-popup")
            .occlude()
            .absolute()
            .left_0()
            .right(px(POPUP_RIGHT_INSET))
            .max_w(px(POPUP_MAX_WIDTH))
            .flex()
            .flex_col()
            .p(px(FRAME_INSET))
            .shadow(popup_lift())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(ui_controls::tooltip_frame(assets.tooltip_fill.clone()))
            .child(list)
            .child(footer(results.kind));
        // the rise is on `bottom`, so the list appears to unfold from the field
        // rather than slide across the transcript
        let rest = px(COMPOSER_HEIGHT + POPUP_GAP);
        if closing {
            return popup
                .with_animation(
                    ("join-command-close", self.command_epoch),
                    Animation::new(COMMAND_CLOSE_DURATION).with_easing(ease_in_out),
                    move |popup, delta| {
                        popup
                            .opacity(1.0 - delta)
                            .bottom(rest - px(POPUP_RISE * delta))
                    },
                )
                .into_any_element();
        }
        popup
            .with_animation(
                ("join-command-open", self.command_epoch),
                Animation::new(COMMAND_OPEN_DURATION).with_easing(ease_in_out),
                move |popup, delta| {
                    popup
                        .opacity(delta)
                        .bottom(rest - px(POPUP_RISE * (1.0 - delta)))
                },
            )
            .into_any_element()
    }
}

fn channel_row(
    index: usize,
    row: &JoinRow,
    query: &str,
    selected: bool,
    tint: CommandTint,
    cx: &mut Context<SuperiorityView>,
) -> Stateful<Div> {
    let mut item = div()
        .id(("join-command-row", index))
        .h(px(ROW_HEIGHT))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(10.0))
        .px(px(ROW_INSET))
        .border_l(px(RAIL_WIDTH))
        .border_color(rgba(0x0000_0000))
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, window, cx| {
            this.take_command_row(index, window, cx);
        }));
    if selected {
        item = item
            .bg(rgba(tint.selected_fill))
            .border_color(rgb(tint.accent));
    } else {
        item = item.hover(move |style| style.bg(rgba(tint.hover_fill)));
    }
    // an empty room is still worth offering, but it is not worth reading first
    if row.dead() {
        item = item.opacity(ABSENT_OPACITY);
    }
    // a group carries its own icon; a channel has none, and a generic glyph
    // repeated down every row would be filler
    if matches!(row.source, JoinSource::Group | JoinSource::Community) {
        item = item.child(
            img(row.icon)
                .size(px(ICON_SIZE))
                .flex_shrink_0()
                .border_1()
                .border_color(rgba(0x33a8_f080))
                .object_fit(ObjectFit::Cover),
        );
    } else {
        item = item.child(div().w(px(ICON_SIZE)).flex_shrink_0());
    }
    item = item.child(highlighted_name(
        &row.name,
        query,
        if selected { tint.selected_text } else { TEXT },
        tint,
    ));
    // the kind rides on the row, since there are no section headers to carry it
    if let Some(note) = &row.note {
        item = item.child(
            div()
                .flex_shrink_0()
                .font_family(FONT_INTERFACE)
                .text_size(px(10.0))
                .text_color(rgb(KEY))
                .child(note.to_uppercase()),
        );
    }
    item.child(
        div()
            .w(px(COUNT_WIDTH))
            .flex_shrink_0()
            .flex()
            .justify_end()
            .font_family(FONT_INTERFACE)
            .text_size(px(11.0))
            .text_color(
                row.count
                    .map_or(rgb(MUTED), |count| rgb(count_color(count))),
            )
            .child(row.count.map(|count| count.to_string()).unwrap_or_default()),
    )
}

/// the name with the part you typed underlined, so a list of near-identical
/// names shows you why each one is here.
fn highlighted_name(name: &str, query: &str, color: u32, tint: CommandTint) -> Div {
    let line = div()
        .flex_1()
        .min_w_0()
        .flex()
        .overflow_hidden()
        .whitespace_nowrap()
        .font_family(FONT_INTERNATIONAL)
        .text_size(px(13.0))
        .text_color(rgb(color));
    let Some(span) = match_span(name, query) else {
        return line.child(name.to_owned());
    };
    line.child(name[..span.start].to_owned())
        .child(
            div()
                .flex_shrink_0()
                .text_color(rgb(tint.accent))
                .underline()
                .child(name[span.start..span.end].to_owned()),
        )
        .child(name[span.end..].to_owned())
}

/// the last line offers to make the room when nothing answers to the name. it
/// is drawn as an action rather than a result — a rule above it, orange, and
/// shorter than the rows it follows.
fn create_row(
    query: &str,
    index: usize,
    selected: bool,
    tint: CommandTint,
    cx: &mut Context<SuperiorityView>,
) -> Stateful<Div> {
    let mut item = div()
        .id("join-command-create")
        .h(px(CREATE_HEIGHT))
        .flex_shrink_0()
        .mt(px(4.0))
        .flex()
        .items_center()
        .gap(px(10.0))
        .px(px(ROW_INSET))
        .border_t_1()
        .border_l(px(RAIL_WIDTH))
        .border_color(rgba(BORDER_STRUCTURAL))
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, window, cx| {
            this.take_command_row(index, window, cx);
        }));
    if selected {
        item = item.bg(rgba(tint.selected_fill));
    } else {
        item = item.hover(move |style| style.bg(rgba(tint.hover_fill)));
    }
    item.child(
        div()
            .w(px(ICON_SIZE))
            .flex_shrink_0()
            .flex()
            .justify_center()
            .font_family(FONT_INTERFACE)
            .text_size(px(12.0))
            .text_color(rgb(CREATE))
            .child("+"),
    )
    .child(
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .gap(px(4.0))
            .overflow_hidden()
            .whitespace_nowrap()
            .font_family(FONT_INTERFACE)
            .text_size(px(11.5))
            .text_color(rgb(KEY))
            .child("Create channel")
            .child(
                div()
                    .font_family(FONT_INTERNATIONAL)
                    .text_color(rgb(CREATE))
                    .child(format!("\"{query}\"")),
            ),
    )
}

/// one person: the face the roster would show, the name, and one line saying
/// why they are in this list at all.
fn person_row(
    index: usize,
    row: &PersonRow,
    results: &CommandResults,
    selected: bool,
    tint: CommandTint,
    assets: &UiAssets,
    cx: &mut Context<SuperiorityView>,
) -> Stateful<Div> {
    let mut item = div()
        .id(("join-command-row", index))
        .h(px(PERSON_ROW_HEIGHT))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(10.0))
        .px(px(ROW_INSET))
        .border_l(px(RAIL_WIDTH))
        .border_color(rgba(0x0000_0000))
        .cursor_pointer()
        .on_click(cx.listener({
            let action = index;
            move |this, _, window, cx| {
                this.take_command_row(action, window, cx);
            }
        }));
    if selected {
        item = item
            .bg(rgba(tint.selected_fill))
            .border_color(rgb(tint.accent));
    } else {
        item = item.hover(move |style| style.bg(rgba(tint.hover_fill)));
    }
    // one dim value for the whole row, so an offline person reads as one
    // greyed object rather than a row of separately faded parts
    if row.offline {
        item = item.opacity(ABSENT_OPACITY);
    }
    item.child(ui_roster::framed_portrait(
        row.portrait.as_ref(),
        assets,
        PORTRAIT_FRAME,
        PORTRAIT_FACE,
    ))
    .child(
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .child(person_name(row, &results.query, selected, tint))
            .child(
                div()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .font_family(FONT_INTERFACE)
                    .text_size(px(10.0))
                    .text_color(rgb(if selected { tint.accent } else { MUTED }))
                    .child(row.context.clone()),
            ),
    )
    .child(
        div()
            .size(px(DOT_SIZE))
            .flex_shrink_0()
            .rounded_full()
            .bg(rgb(row.presence.dot_color()))
            // present states glow; absent ones sit flat, so a list of away
            // players never lights up the popup
            .when(row.presence.dot_glows(), |dot| {
                dot.shadow(vec![
                    gpui::BoxShadow::new(
                        px(0.0),
                        px(0.0),
                        rgba((row.presence.dot_color() << 8) | DOT_GLOW_ALPHA).into(),
                    )
                    .blur_radius(px(5.0)),
                ])
            }),
    )
}

fn person_name(row: &PersonRow, query: &str, selected: bool, tint: CommandTint) -> Div {
    div()
        .flex()
        .gap(px(4.0))
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .when_some(row.clan_tag.clone(), |line, tag| {
            line.child(
                div()
                    .flex_shrink_0()
                    .font_family(FONT_INTERNATIONAL)
                    .text_size(px(12.5))
                    .text_color(rgb(if row.own_clan { CREATE } else { KEY }))
                    .child(format!("<{tag}>")),
            )
        })
        .child(
            highlighted_name(
                &row.name,
                query,
                if selected { tint.selected_text } else { TEXT },
                tint,
            )
            .text_size(px(12.5)),
        )
}

/// the keys the popup answers to, spelled out so the fast path is discoverable
/// the first time you stumble into it.
fn footer(kind: CommandKind) -> Div {
    div()
        .h(px(FOOTER_HEIGHT))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(14.0))
        .px(px(ROW_INSET))
        .border_t_1()
        .border_color(rgba(BORDER_STRUCTURAL))
        .font_family(FONT_INTERFACE)
        .text_size(px(9.5))
        .text_color(rgb(MUTED))
        .child(hint("\u{2191}\u{2193}", "navigate"))
        // a mention is taken by either key, so they are named together rather
        // than given a line each
        .when(kind == CommandKind::Mention, |footer| {
            footer.child(hint("\u{21b5} tab", kind.confirm_label()))
        })
        .when(kind != CommandKind::Mention, |footer| {
            footer.child(hint("\u{21b5}", kind.confirm_label()))
        })
        // completing a name in place only means something when the name is the
        // whole line
        .when(kind == CommandKind::Join, |footer| {
            footer.child(hint("tab", "complete"))
        })
        .child(hint("esc", "dismiss"))
}

fn hint(key: &'static str, action: &'static str) -> Div {
    div()
        .flex()
        .gap(px(4.0))
        .child(div().text_color(rgb(KEY)).child(key))
        .child(action)
}
