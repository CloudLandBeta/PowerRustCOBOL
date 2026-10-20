use super::*;

pub(super) fn compute_timeline_layout(
    graph: &Graph,
    theme: &Theme,
    config: &LayoutConfig,
) -> Layout {
    let data = &graph.timeline;
    let font_size = theme.font_size;
    let padding = 30.0;
    let event_width = 120.0;
    let event_height = 80.0;
    let event_spacing = 40.0;
    let title_height = if data.title.is_some() { 40.0 } else { 0.0 };
    let line_y = padding + title_height + 60.0;

    let num_events = data.events.len().max(1);
    let total_events_width =
        num_events as f32 * event_width + (num_events - 1).max(0) as f32 * event_spacing;

    let width = padding * 2.0 + total_events_width;
    let height = padding * 2.0 + title_height + event_height + 100.0;

    let title = data.title.as_ref().map(|t| measure_label(t, theme, config));

    let events: Vec<TimelineEventLayout> = data
        .events
        .iter()
        .enumerate()
        .map(|(i, event)| {
            let x = padding + i as f32 * (event_width + event_spacing);
            let y = line_y + 30.0;

            let time_block = measure_label(&event.time, theme, config);
            let event_blocks: Vec<TextBlock> = event
                .events
                .iter()
                .map(|e| measure_label(e, theme, config))
                .collect();

            TimelineEventLayout {
                time: time_block,
                events: event_blocks,
                x,
                y,
                width: event_width,
                height: event_height,
                circle_y: line_y,
            }
        })
        .collect();

    let line_start_x = padding;
    let line_end_x = width - padding;

    // Sections (simplified - just record them)
    let sections: Vec<TimelineSectionLayout> = data
        .sections
        .iter()
        .enumerate()
        .map(|(i, section)| {
            let label = measure_label(section, theme, config);
            TimelineSectionLayout {
                label,
                x: padding + i as f32 * 200.0,
                y: padding,
                width: 180.0,
                height: 30.0,
            }
        })
        .collect();

    let mut nodes = BTreeMap::new();
    nodes.insert(
        "__timeline_metrics_content".to_string(),
        NodeLayout {
            id: "__timeline_metrics_content".to_string(),
            x: padding,
            y: padding,
            width: (width - padding * 2.0).max(1.0),
            height: (height - padding * 2.0).max(1.0),
            label: TextBlock {
                lines: vec![String::new()],
                width: 0.0,
                height: 0.0,
            },
            shape: crate::ir::NodeShape::Rectangle,
            style: crate::ir::NodeStyle::default(),
            link: None,
            anchor_subgraph: None,
            hidden: false,
            icon: None,
        },
    );

    Layout {
        kind: graph.kind,
        nodes,
        edges: Vec::new(),
        subgraphs: Vec::new(),
        diagram: DiagramData::Timeline(TimelineLayout {
            title,
            title_y: padding + font_size,
            events,
            sections,
            line_y,
            line_start_x,
            line_end_x,
            width,
            height,
        }),
        width,
        height,
    }
}
