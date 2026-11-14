//! Layer overview for `keyloom layers`; the full layer graph is in `explore`.

use crate::model::{Layout, Slot};
use crate::positions::{display_name, PositionMap};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerSummary {
    pub position: i64,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Keys that do something on this layer (not transparent, not KC_NO).
    pub bound: usize,
    pub transparent: usize,
    /// Keys on other layers that lead here.
    pub reached_by: Vec<LayerLink>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerLink {
    pub from_layer: i64,
    pub from_title: String,
    pub key: String,
    pub slot: Slot,
    /// `MO`, `TG`, `TO`, …
    pub action: String,
}

impl LayerLink {
    /// e.g. "hold L.thumb.2 on Main (MO)".
    pub fn describe(&self) -> String {
        let slot = match self.slot {
            Slot::Tap => "tap",
            Slot::Hold => "hold",
            Slot::DoubleTap => "double-tap",
            Slot::TapHold => "tap-then-hold",
        };
        format!("{slot} {} on {} ({})", self.key, self.from_title, self.action)
    }
}

pub fn layers(layout: &Layout) -> Vec<LayerSummary> {
    let map = PositionMap::for_geometry(&layout.geometry);
    layout
        .layers
        .iter()
        .map(|layer| {
            let mut reached_by = Vec::new();
            for from in &layout.layers {
                for (i, key) in from.keys.iter().enumerate() {
                    for (slot, action) in key.slots() {
                        if action.layer == Some(layer.position) && from.position != layer.position {
                            reached_by.push(LayerLink {
                                from_layer: from.position,
                                from_title: from.title.clone(),
                                key: display_name(map.as_ref(), i),
                                slot,
                                action: action.code().unwrap_or("?").to_string(),
                            });
                        }
                    }
                }
            }
            let transparent = if layer.position == 0 { 0 } else { layer.keys.iter().filter(|k| k.is_transparent()).count() };
            let bound = layer.keys.iter().filter(|k| !(layer.position != 0 && k.is_transparent()) && !k.is_no() && !k.is_blank()).count();
            LayerSummary {
                position: layer.position,
                title: layer.title.clone(),
                color: layer.color.clone(),
                tags: layer.keyloom.tags.clone(),
                bound,
                transparent,
                reached_by,
            }
        })
        .collect()
}
