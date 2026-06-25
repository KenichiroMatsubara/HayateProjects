use hayate_core::element::taffy_bridge::apply_to_style;
use hayate_core::{
    AlignContentValue, AlignSelfValue, BoxSizingValue, Dimension, FlexWrapValue, PositionValue,
    StyleProp,
};
use taffy::prelude::*;

#[test]
fn flex_shrink_maps_to_taffy_style() {
    let mut style = Style::default();
    assert!(apply_to_style(
        &mut style,
        &StyleProp::FlexShrink(2.5)
    ));
    assert!((style.flex_shrink - 2.5).abs() < f32::EPSILON);
}

#[test]
fn flex_basis_maps_to_taffy_style() {
    let mut style = Style::default();
    assert!(apply_to_style(
        &mut style,
        &StyleProp::FlexBasis(Dimension::px(120.0))
    ));
    match style.flex_basis {
        taffy::style::Dimension::Length(v) => assert!((v - 120.0).abs() < f32::EPSILON),
        other => panic!("expected Length, got {other:?}"),
    }
}

#[test]
fn align_self_maps_to_taffy_style() {
    let mut style = Style::default();
    assert!(apply_to_style(
        &mut style,
        &StyleProp::AlignSelf(AlignSelfValue::Center)
    ));
    assert_eq!(style.align_self, Some(AlignItems::Center));
}

#[test]
fn align_self_auto_clears_taffy_override() {
    let mut style = Style::default();
    style.align_self = Some(AlignItems::FlexEnd);
    assert!(apply_to_style(
        &mut style,
        &StyleProp::AlignSelf(AlignSelfValue::Auto)
    ));
    assert_eq!(style.align_self, None);
}

#[test]
fn flex_wrap_maps_to_taffy_style() {
    let mut style = Style::default();
    assert!(apply_to_style(
        &mut style,
        &StyleProp::FlexWrap(FlexWrapValue::Wrap)
    ));
    assert_eq!(style.flex_wrap, FlexWrap::Wrap);
}

#[test]
fn flex_wrap_reverse_maps_to_taffy_style() {
    let mut style = Style::default();
    assert!(apply_to_style(
        &mut style,
        &StyleProp::FlexWrap(FlexWrapValue::WrapReverse)
    ));
    assert_eq!(style.flex_wrap, FlexWrap::WrapReverse);
}

#[test]
fn position_absolute_maps_to_taffy_style() {
    let mut style = Style::default();
    assert!(apply_to_style(
        &mut style,
        &StyleProp::Position(PositionValue::Absolute)
    ));
    assert_eq!(style.position, Position::Absolute);
}

#[test]
fn position_relative_maps_to_taffy_style() {
    let mut style = Style::default();
    style.position = Position::Absolute;
    assert!(apply_to_style(
        &mut style,
        &StyleProp::Position(PositionValue::Relative)
    ));
    assert_eq!(style.position, Position::Relative);
}

#[test]
fn inset_maps_each_edge_to_taffy_style() {
    let mut style = Style::default();
    assert!(apply_to_style(&mut style, &StyleProp::Top(Dimension::px(10.0))));
    assert!(apply_to_style(&mut style, &StyleProp::Left(Dimension::px(20.0))));
    assert!(apply_to_style(&mut style, &StyleProp::Right(Dimension::px(30.0))));
    assert!(apply_to_style(
        &mut style,
        &StyleProp::Bottom(Dimension::px(40.0))
    ));
    assert_eq!(style.inset.top, LengthPercentageAuto::Length(10.0));
    assert_eq!(style.inset.left, LengthPercentageAuto::Length(20.0));
    assert_eq!(style.inset.right, LengthPercentageAuto::Length(30.0));
    assert_eq!(style.inset.bottom, LengthPercentageAuto::Length(40.0));
}

#[test]
fn aspect_ratio_maps_to_taffy_style() {
    let mut style = Style::default();
    assert!(apply_to_style(&mut style, &StyleProp::AspectRatio(1.5)));
    assert_eq!(style.aspect_ratio, Some(1.5));
}

#[test]
fn aspect_ratio_non_positive_clears_taffy_override() {
    let mut style = Style::default();
    style.aspect_ratio = Some(2.0);
    // 0 や負の比率は無効。Taffy には書き込まず無効化する。
    assert!(apply_to_style(&mut style, &StyleProp::AspectRatio(0.0)));
    assert_eq!(style.aspect_ratio, None);
}

#[test]
fn aspect_ratio_is_a_layout_prop() {
    // レイアウト系なので Taffy へ流れる（Visual には入らない）。
    assert!(StyleProp::AspectRatio(1.5).is_layout());
}

#[test]
fn box_sizing_maps_each_value_to_taffy_style() {
    let mut style = Style::default();
    assert!(apply_to_style(
        &mut style,
        &StyleProp::BoxSizing(BoxSizingValue::ContentBox)
    ));
    assert_eq!(style.box_sizing, BoxSizing::ContentBox);

    assert!(apply_to_style(
        &mut style,
        &StyleProp::BoxSizing(BoxSizingValue::BorderBox)
    ));
    assert_eq!(style.box_sizing, BoxSizing::BorderBox);
}

#[test]
fn box_sizing_is_a_layout_prop() {
    // レイアウト系なので Taffy へ流れる（Visual には入らない）。
    assert!(StyleProp::BoxSizing(BoxSizingValue::ContentBox).is_layout());
}

#[test]
fn align_content_maps_to_taffy_style() {
    let mut style = Style::default();
    assert!(apply_to_style(
        &mut style,
        &StyleProp::AlignContent(AlignContentValue::SpaceBetween)
    ));
    assert_eq!(style.align_content, Some(AlignContent::SpaceBetween));
}

#[test]
fn grid_auto_rows_maps_to_taffy_style() {
    let mut style = Style::default();
    assert!(apply_to_style(
        &mut style,
        &StyleProp::GridAutoRows(vec![Dimension::px(30.0), Dimension::fr(1.0)])
    ));
    assert_eq!(style.grid_auto_rows.len(), 2);
    assert_eq!(style.grid_auto_rows[0], length(30.0));
    assert_eq!(style.grid_auto_rows[1], fr(1.0));
    // 暗黙トラックは grid_auto_* に入り、明示トラック grid_template_* は触らない。
    assert!(style.grid_template_rows.is_empty());
}

#[test]
fn grid_auto_columns_maps_to_taffy_style() {
    let mut style = Style::default();
    assert!(apply_to_style(
        &mut style,
        &StyleProp::GridAutoColumns(vec![Dimension::percent(50.0)])
    ));
    assert_eq!(style.grid_auto_columns.len(), 1);
    assert_eq!(style.grid_auto_columns[0], percent(0.5));
    assert!(style.grid_template_columns.is_empty());
}

#[test]
fn grid_auto_tracks_are_layout_props() {
    // レイアウト系なので Taffy へ流れる（Visual には入らない）。
    assert!(StyleProp::GridAutoRows(vec![Dimension::px(30.0)]).is_layout());
    assert!(StyleProp::GridAutoColumns(vec![Dimension::px(30.0)]).is_layout());
}

#[test]
fn grid_auto_flow_maps_each_value_to_taffy_style() {
    use hayate_core::GridAutoFlowValue;

    let cases = [
        (GridAutoFlowValue::Row, GridAutoFlow::Row),
        (GridAutoFlowValue::Column, GridAutoFlow::Column),
        (GridAutoFlowValue::RowDense, GridAutoFlow::RowDense),
        (GridAutoFlowValue::ColumnDense, GridAutoFlow::ColumnDense),
    ];
    for (input, expected) in cases {
        let mut style = Style::default();
        assert!(apply_to_style(&mut style, &StyleProp::GridAutoFlow(input)));
        assert_eq!(style.grid_auto_flow, expected);
    }
}

#[test]
fn grid_auto_flow_is_a_layout_prop() {
    // 自動配置の主軸・詰め方はレイアウト系なので Taffy へ流れる（Visual には入らない）。
    assert!(StyleProp::GridAutoFlow(hayate_core::GridAutoFlowValue::RowDense).is_layout());
}
