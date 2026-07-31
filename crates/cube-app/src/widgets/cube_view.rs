//! The interactive 3D cube widget: drag to orbit, scroll/pinch to zoom,
//! renders via the egui-wgpu callback.

use cube_core::{FaceletCube, Move};
use cube_render::{
    instances_from_state, mask_for_move, CubeCallback, FrameData, MoveAnimator, OrbitCamera,
};
use egui::{Response, Sense, Ui, Vec2};

pub struct CubeView<'a> {
    pub cube: &'a FaceletCube,
    pub animator: &'a MoveAnimator,
    pub orbit: &'a mut OrbitCamera,
    /// Layer to pulse-highlight (the next move a user should perform).
    pub highlight: Option<Move>,
    /// How strongly the rest of the cube desaturates while something is
    /// highlighted: 1.0 = selection (Play/lessons), ~0.3 = guide hint.
    pub dim_others: f32,
    /// Per-facelet palette override (scan preview): index -> palette color.
    pub color_override: Option<&'a dyn Fn(usize) -> Option<u32>>,
}

impl CubeView<'_> {
    pub fn show(self, ui: &mut Ui, size: Vec2) -> Response {
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        if response.dragged() {
            let d = response.drag_delta();
            self.orbit.drag(d.x, d.y);
        }
        if response.hovered() {
            // Wheel scroll and pinch both zoom.
            let scroll = ui.input(|i| i.smooth_scroll_delta.y + (i.zoom_delta() - 1.0) * 600.0);
            if scroll.abs() > 0.1 {
                self.orbit.zoom(scroll);
            }
        }
        let now = ui.input(|i| i.time);
        let pose = self.animator.pose(now);
        let highlight_mask = self.highlight.map(mask_for_move).unwrap_or(0);
        let ppp = ui.ctx().pixels_per_point();
        let frame = FrameData {
            view_proj: self.orbit.view_proj(rect.aspect_ratio()),
            time: now as f32,
            dim: self.dim_others,
            target_px: [
                (rect.width() * ppp).round().max(1.0) as u32,
                (rect.height() * ppp).round().max(1.0) as u32,
            ],
            instances: instances_from_state(
                self.cube,
                pose.as_ref(),
                highlight_mask,
                self.color_override,
            ),
        };
        ui.painter()
            .add(egui_wgpu::Callback::new_paint_callback(rect, CubeCallback(frame)));
        response
    }
}
