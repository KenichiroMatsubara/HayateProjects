use std::collections::HashMap;

use crate::color::Color;
use crate::element::id::ElementId;
use crate::element::tree::{Element, Visual};

/// Ambient Default Text Style channel (ADR-0065 ch2). Block-penetrating defaults
/// supplied by `default-*` style props on any ancestor.
#[derive(Clone, Debug)]
pub struct AmbientDefaults {
    pub color: Color,
    pub font_size: f32,
    pub font_family: Option<String>,
    pub font_weight: Option<f32>,
}

impl Default for AmbientDefaults {
    fn default() -> Self {
        Self::hard()
    }
}

impl AmbientDefaults {
    pub fn hard() -> Self {
        Self {
            color: Color::BLACK,
            font_size: 16.0,
            font_family: None,
            font_weight: Some(400.0),
        }
    }

    pub fn merge_visual(&self, visual: &Visual) -> Self {
        Self {
            color: visual.default_color.unwrap_or(self.color),
            font_size: visual.default_font_size.unwrap_or(self.font_size),
            font_family: visual
                .default_font_family
                .clone()
                .or_else(|| self.font_family.clone()),
            font_weight: visual.default_font_weight.or(self.font_weight),
        }
    }
}

/// Resolve ambient defaults at `id` by walking root→id and merging `default-*` props.
pub(crate) fn ambient_at(elements: &HashMap<ElementId, Element>, id: ElementId) -> AmbientDefaults {
    let mut chain = Vec::new();
    let mut cur = Some(id);
    while let Some(eid) = cur {
        chain.push(eid);
        cur = elements.get(&eid).and_then(|e| e.parent);
    }
    chain.reverse();
    let mut ambient = AmbientDefaults::hard();
    for eid in chain {
        if let Some(el) = elements.get(&eid) {
            ambient = ambient.merge_visual(&el.visual);
        }
    }
    ambient
}
