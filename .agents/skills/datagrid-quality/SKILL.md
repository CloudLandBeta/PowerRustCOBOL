---
name: datagrid-quality
description: Use when updating or reviewing code that affects DataGrid behavior, rendering, layout, styling, virtualization, embedded controls, data binding, or look-and-feel. This invokes the DataGrid Quality & Compatibility Agent as a guardian before completing the change.
---

# /datagrid-quality — DataGrid Quality & Compatibility Agent

Read `.agents/agents/datagrid-quality-compatibility-agent.md` first, then apply
its checklist to the current change.

## Required use

Use this skill whenever code changes or reviews touch DataGrid behavior or
look-and-feel, including:

- DataGrid rendering, layout, scrolling, virtualization, or clipping;
- row/column sizing, resizing, frozen panes, filters, selection, copy, or CSV;
- grid backgrounds, cell backgrounds, patterns, images, borders, rounded corners,
  glass effects, hover/focus/selection/disabled states, or grid lines;
- embedded controls inside grid cells;
- data-binding metadata, generated binding rows, or refresh behavior;
- theme, event, mouse, keyboard, or accessibility changes that affect grids.

## Workflow

1. Identify the affected DataGrid surfaces: Designer, Preview, Run Form,
   compiled/runtime, codegen, IDE property/editor panels, and data binding.
2. Run the agent checklist against the change.
3. Require targeted automated tests when the affected behavior is testable.
4. Require manual/visual verification when the affected behavior depends on
   pixels, interaction, or screenshots.
5. Block completion if a regression is found and no corrective task/test exists.

## Report

Summarize:

- which DataGrid areas were affected;
- which checklist items were validated;
- tests run and results;
- remaining manual checks or risks.
