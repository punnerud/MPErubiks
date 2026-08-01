//! GPU rendering of the cube: 27 instanced cubies drawn into an offscreen
//! color+depth target inside `CallbackTrait::prepare` (egui's own render
//! pass has no depth buffer), blitted into the widget rect in `paint`.
//! No compute shaders or storage buffers — the WebGL2 fallback must work.

mod animator;
mod callback;
mod camera;
mod math;
mod pick;
mod scene;

pub use animator::{LayerPose, MoveAnimator};
pub use callback::{CubeCallback, CubeRenderResources, FrameData};
pub use camera::OrbitCamera;
pub use pick::{pick_face, pick_layer_by_cell, view_relative_arrows, ArrowDir};
pub use scene::{instances_from_state, mask_for_move, Instance, COLOR_INTERIOR, COLOR_UNKNOWN};
