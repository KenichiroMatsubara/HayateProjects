use hayate_core::element::taffy_bridge::apply_to_style;
use hayate_core::{AlignContentValue, AlignSelfValue, Dimension, FlexWrapValue, StyleProp};
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
fn align_content_maps_to_taffy_style() {
    let mut style = Style::default();
    assert!(apply_to_style(
        &mut style,
        &StyleProp::AlignContent(AlignContentValue::SpaceBetween)
    ));
    assert_eq!(style.align_content, Some(AlignContent::SpaceBetween));
}
