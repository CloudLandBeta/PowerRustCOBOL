use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

use crate::config::LayoutConfig;
use crate::ir::Graph;
use crate::theme::Theme;

use super::text::measure_label_with_font_size;
use super::{
    DiagramData, Layout, PieData, PieLegendItem, PieSliceLayout, PieTitleLayout, TextBlock,
};

fn pie_palette(theme: &Theme) -> Vec<String> {
    theme.pie_colors.to_vec()
}

#[allow(dead_code)]
fn format_pie_value(value: f32) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    if (rounded - rounded.round()).abs() < 0.001 {
        format!("{:.0}", rounded)
    } else {
        format!("{:.2}", rounded)
    }
}

pub(super) fn compute_pie_layout(graph: &Graph, theme: &Theme, config: &LayoutConfig) -> Layout {
    let pie_cfg = &config.pie;
    let mut slices = Vec::new();
    let mut legend = Vec::new();
    let title_block = graph.pie_title.as_ref().map(|title| {
        measure_label_with_font_size(
            title,
            theme.pie_title_text_size,
            config,
            false,
            theme.font_family.as_str(),
        )
    });

    let palette = pie_palette(theme);
    let total: f32 = graph
        .pie_slices
        .iter()
        .map(|slice| slice.value.max(0.0))
        .sum();
    let fallback_total = graph.pie_slices.len().max(1) as f32;
    let total = if total > 0.0 { total } else { fallback_total };

    #[derive(Clone)]
    struct PieDatum {
        index: usize,
        label: String,
        value: f32,
    }

    let mut filtered: Vec<PieDatum> = Vec::new();
    for (idx, slice) in graph.pie_slices.iter().enumerate() {
        let value = slice.value.max(0.0);
        let percent = if total > 0.0 {
            value / total * 100.0
        } else {
            0.0
        };
        if percent >= pie_cfg.min_percent {
            filtered.push(PieDatum {
                index: idx,
                label: slice.label.clone(),
                value,
            });
        }
    }
    filtered.sort_by(|a, b| {
        b.value
            .partial_cmp(&a.value)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.index.cmp(&b.index))
    });

    let mut color_map: HashMap<String, String> = HashMap::new();
    let mut color_index: usize = 0;
    let mut resolve_color = |label: &str| -> String {
        if let Some(color) = color_map.get(label) {
            return color.clone();
        }
        let color = palette[color_index % palette.len()].clone();
        color_index += 1;
        color_map.insert(label.to_string(), color.clone());
        color
    };

    let mut angle = 0.0_f32;
    for datum in &filtered {
        let span = if total > 0.0 {
            datum.value / total * std::f32::consts::PI * 2.0
        } else {
            std::f32::consts::PI * 2.0 / fallback_total
        };
        let label = measure_label_with_font_size(
            &datum.label,
            theme.pie_section_text_size,
            config,
            false,
            theme.font_family.as_str(),
        );
        let color = resolve_color(&datum.label);
        slices.push(PieSliceLayout {
            label,
            value: datum.value,
            start_angle: angle,
            end_angle: angle + span,
            color,
        });
        angle += span;
    }

    let mut legend_width: f32 = 0.0;
    let mut legend_items: Vec<(TextBlock, String)> = Vec::new();
    for slice in &graph.pie_slices {
        let value_text = format_pie_value(slice.value);
        let label_text = if graph.pie_show_data {
            format!("{} [{}]", slice.label, value_text)
        } else {
            slice.label.clone()
        };
        let label = measure_label_with_font_size(
            &label_text,
            theme.pie_legend_text_size,
            config,
            false,
            theme.font_family.as_str(),
        );
        legend_width = legend_width.max(label.width);
        let color = resolve_color(&slice.label);
        legend_items.push((label, color));
    }

    let legend_text_height = theme.pie_legend_text_size * 1.25;
    let legend_item_height =
        (pie_cfg.legend_rect_size + pie_cfg.legend_spacing).max(legend_text_height);
    let legend_offset = legend_item_height * legend_items.len() as f32 / 2.0;

    let height = pie_cfg.height.max(1.0);
    let pie_width = height;
    let radius = (pie_width.min(height) / 2.0 - pie_cfg.margin).max(1.0);
    let center_x = pie_width / 2.0;
    let center_y = height / 2.0;
    let legend_x = center_x + radius + pie_cfg.margin * 0.6;

    for (idx, (label, color)) in legend_items.into_iter().enumerate() {
        let vertical = idx as f32 * legend_item_height - legend_offset;
        legend.push(PieLegendItem {
            x: legend_x,
            y: center_y + vertical,
            label,
            color,
            marker_size: pie_cfg.legend_rect_size,
            value: graph.pie_slices[idx].value,
        });
    }

    let width = legend_x
        + pie_cfg.legend_rect_size
        + pie_cfg.legend_spacing
        + legend_width
        + pie_cfg.margin * 0.4;
    let title_layout = title_block.map(|text| PieTitleLayout {
        x: center_x,
        y: center_y - (height - 50.0) / 2.0,
        text,
    });

    Layout {
        kind: graph.kind,
        nodes: BTreeMap::new(),
        edges: Vec::new(),
        subgraphs: Vec::new(),
        width: width.max(200.0),
        height: height.max(1.0),
        diagram: DiagramData::Pie(PieData {
            slices,
            legend,
            center: (center_x, center_y),
            radius,
            title: title_layout,
        }),
    }
}
