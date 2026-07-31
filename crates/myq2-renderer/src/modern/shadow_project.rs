//! Projective (depth-based) dynamic shadows for entities/movers.
//!
//! Renders every dynamic shadow caster (alias models — player, pickups, projectiles — plus
//! sprites and particles) into a single directional depth shadow map from an angled-down
//! light. A fullscreen resolve pass then reconstructs each scene pixel's world position from
//! the depth buffer, projects it into the light's clip space, and darkens it if a caster is
//! nearer the light along that ray. Because the test uses each pixel's REAL world depth, the
//! shadow drapes over and splits across whatever geometry is below/around the caster — floor,
//! wall, ledge, deep floor — instead of being flattened onto one plane like the blob shadow.
//!
//! This module owns only the math + (later) the Vulkan resources; render_path drives it.

/// Column-major 4x4 multiply: returns a*b (so (a*b)*v applies b then a), matching the
/// renderer's other [f32;16] matrices.
pub fn mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut o = [0.0f32; 16];
    for col in 0..4 {
        for row in 0..4 {
            let mut s = 0.0;
            for k in 0..4 {
                s += a[k * 4 + row] * b[col * 4 + k];
            }
            o[col * 4 + row] = s;
        }
    }
    o
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 1e-6 { [v[0] / l, v[1] / l, v[2] / l] } else { [0.0, 0.0, 1.0] }
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }

/// Right-handed look-at (column-major), matching render_path's look_at_rh.
fn look_at(eye: [f32; 3], center: [f32; 3], up: [f32; 3]) -> [f32; 16] {
    let f = normalize([center[0] - eye[0], center[1] - eye[1], center[2] - eye[2]]);
    let s = normalize(cross(f, up));
    let u = cross(s, f);
    [
        s[0], u[0], -f[0], 0.0,
        s[1], u[1], -f[1], 0.0,
        s[2], u[2], -f[2], 0.0,
        -dot(s, eye), -dot(u, eye), dot(f, eye), 1.0,
    ]
}

/// Right-handed orthographic projection into Vulkan clip space (z in [0,1], y down handled
/// by the same convention as the renderer's perspective). Column-major.
fn ortho(half_extent: f32, near: f32, far: f32) -> [f32; 16] {
    let r = half_extent;
    let t = half_extent;
    // x: [-r,r]->[-1,1], y: [-t,t]->[-1,1], z: [near,far]->[0,1]
    [
        1.0 / r, 0.0,      0.0,                 0.0,
        0.0,     1.0 / t,  0.0,                 0.0,
        0.0,     0.0,      -1.0 / (far - near), 0.0,
        0.0,     0.0,      -near / (far - near), 1.0,
    ]
}

/// Build the directional shadow light's view-projection: an orthographic box centred on the
/// area around the viewer, looking down along `light_dir`. `coverage` is the half-width of
/// the box (world units); `height` is how far above the focus the light eye sits.
pub fn light_view_proj(focus: [f32; 3], light_dir: [f32; 3], coverage: f32, height: f32) -> [f32; 16] {
    let dir = normalize(light_dir);
    let eye = [focus[0] - dir[0] * height, focus[1] - dir[1] * height, focus[2] - dir[2] * height];
    // Pick an up vector not parallel to the (mostly vertical) light direction.
    let up = if dir[2].abs() > 0.95 { [0.0, 1.0, 0.0] } else { [0.0, 0.0, 1.0] };
    let view = look_at(eye, focus, up);
    let proj = ortho(coverage, 1.0, height * 2.0 + coverage * 2.0);
    mul(&proj, &view)
}

/// Invert a column-major 4x4 matrix. Returns identity if singular.
pub fn invert(m: &[f32; 16]) -> [f32; 16] {
    let mut inv = [0.0f32; 16];
    inv[0] = m[5]*m[10]*m[15] - m[5]*m[11]*m[14] - m[9]*m[6]*m[15] + m[9]*m[7]*m[14] + m[13]*m[6]*m[11] - m[13]*m[7]*m[10];
    inv[4] = -m[4]*m[10]*m[15] + m[4]*m[11]*m[14] + m[8]*m[6]*m[15] - m[8]*m[7]*m[14] - m[12]*m[6]*m[11] + m[12]*m[7]*m[10];
    inv[8] = m[4]*m[9]*m[15] - m[4]*m[11]*m[13] - m[8]*m[5]*m[15] + m[8]*m[7]*m[13] + m[12]*m[5]*m[11] - m[12]*m[7]*m[9];
    inv[12] = -m[4]*m[9]*m[14] + m[4]*m[10]*m[13] + m[8]*m[5]*m[14] - m[8]*m[6]*m[13] - m[12]*m[5]*m[10] + m[12]*m[6]*m[9];
    inv[1] = -m[1]*m[10]*m[15] + m[1]*m[11]*m[14] + m[9]*m[2]*m[15] - m[9]*m[3]*m[14] - m[13]*m[2]*m[11] + m[13]*m[3]*m[10];
    inv[5] = m[0]*m[10]*m[15] - m[0]*m[11]*m[14] - m[8]*m[2]*m[15] + m[8]*m[3]*m[14] + m[12]*m[2]*m[11] - m[12]*m[3]*m[10];
    inv[9] = -m[0]*m[9]*m[15] + m[0]*m[11]*m[13] + m[8]*m[1]*m[15] - m[8]*m[3]*m[13] - m[12]*m[1]*m[11] + m[12]*m[3]*m[9];
    inv[13] = m[0]*m[9]*m[14] - m[0]*m[10]*m[13] - m[8]*m[1]*m[14] + m[8]*m[2]*m[13] + m[12]*m[1]*m[10] - m[12]*m[2]*m[9];
    inv[2] = m[1]*m[6]*m[15] - m[1]*m[7]*m[14] - m[5]*m[2]*m[15] + m[5]*m[3]*m[14] + m[13]*m[2]*m[7] - m[13]*m[3]*m[6];
    inv[6] = -m[0]*m[6]*m[15] + m[0]*m[7]*m[14] + m[4]*m[2]*m[15] - m[4]*m[3]*m[14] - m[12]*m[2]*m[7] + m[12]*m[3]*m[6];
    inv[10] = m[0]*m[5]*m[15] - m[0]*m[7]*m[13] - m[4]*m[1]*m[15] + m[4]*m[3]*m[13] + m[12]*m[1]*m[7] - m[12]*m[3]*m[5];
    inv[14] = -m[0]*m[5]*m[14] + m[0]*m[6]*m[13] + m[4]*m[1]*m[14] - m[4]*m[2]*m[13] - m[12]*m[1]*m[6] + m[12]*m[2]*m[5];
    inv[3] = -m[1]*m[6]*m[11] + m[1]*m[7]*m[10] + m[5]*m[2]*m[11] - m[5]*m[3]*m[10] - m[9]*m[2]*m[7] + m[9]*m[3]*m[6];
    inv[7] = m[0]*m[6]*m[11] - m[0]*m[7]*m[10] - m[4]*m[2]*m[11] + m[4]*m[3]*m[10] + m[8]*m[2]*m[7] - m[8]*m[3]*m[6];
    inv[11] = -m[0]*m[5]*m[11] + m[0]*m[7]*m[9] + m[4]*m[1]*m[11] - m[4]*m[3]*m[9] - m[8]*m[1]*m[7] + m[8]*m[3]*m[5];
    inv[15] = m[0]*m[5]*m[10] - m[0]*m[6]*m[9] - m[4]*m[1]*m[10] + m[4]*m[2]*m[9] + m[8]*m[1]*m[6] - m[8]*m[2]*m[5];
    let det = m[0]*inv[0] + m[1]*inv[4] + m[2]*inv[8] + m[3]*inv[12];
    if det.abs() < 1e-12 {
        return [1.0,0.0,0.0,0.0, 0.0,1.0,0.0,0.0, 0.0,0.0,1.0,0.0, 0.0,0.0,0.0,1.0];
    }
    let id = 1.0 / det;
    for x in &mut inv { *x *= id; }
    inv
}

/// Default global shadow light direction: mostly straight down with a slight lean. Keeping
/// it near-vertical makes a passing caster's shadow fall onto the ground directly below it
/// (so it sweeps across your view as it passes overhead) instead of streaking diagonally
/// onto distant walls.
pub const DEFAULT_LIGHT_DIR: [f32; 3] = [0.2, 0.15, -1.0];

/// Shadow-map resolution (square). 4096 keeps the per-texel footprint small over the
/// covered area so shadow edges are crisp before filtering.
pub const SHADOW_SIZE: u32 = 4096;
/// Half-width of the area (world units) the shadow map covers around the viewer. Kept tight
/// so the 4096² map has a small per-texel footprint near the player (crisper edges) rather
/// than spreading resolution over a large area. Trade-off: casters past this aren't shadowed.
pub const COVERAGE: f32 = 700.0;
/// How far above the focus the shadow light eye sits.
pub const LIGHT_HEIGHT: f32 = 2000.0;

/// Vulkan resources for the projective shadow system. Created lazily; owned by the render
/// path. `color_image` (R32) stores the casters' light-space depth (sampled by the resolve);
/// `depth_image` (D32) is the depth buffer used while rendering casters.
#[derive(Default)]
pub struct ProjectiveShadow {
    /// Render pass for the caster depth pass: RG32 colour (R=caster depth, G=floor depth)
    /// + D32 depth. Separate from the shared R32 cubemap shadow pass.
    pub caster_rp: Option<ash::vk::RenderPass>,
    pub color_image: Option<ash::vk::Image>,
    pub color_view: Option<ash::vk::ImageView>,
    pub color_mem: Option<ash::vk::DeviceMemory>,
    pub depth_image: Option<ash::vk::Image>,
    pub depth_view: Option<ash::vk::ImageView>,
    pub depth_mem: Option<ash::vk::DeviceMemory>,
    pub framebuffer: Option<ash::vk::Framebuffer>,
    /// Render pass for the resolve pass (one colour attachment = scene colour, LOAD/STORE).
    pub resolve_rp: Option<ash::vk::RenderPass>,
    /// Framebuffer wrapping the scene colour view for the resolve pass; recreated when the
    /// scene colour view changes (resize). `resolve_fb_view` is the view it currently wraps.
    pub resolve_fb: Option<ash::vk::Framebuffer>,
    pub resolve_fb_view: Option<ash::vk::ImageView>,
    /// DEPTH-aspect-only view of the scene depth image for sampling (the scene's own depth
    /// view is a combined DEPTH|STENCIL view, which is NOT sampleable). Recreated when the
    /// scene depth image changes. `depth_sample_src` is the image it currently views.
    pub depth_sample_view: Option<ash::vk::ImageView>,
    pub depth_sample_src: Option<ash::vk::Image>,
    pub resolve_pool: Option<ash::vk::DescriptorPool>,
    pub resolve_set: Option<ash::vk::DescriptorSet>,
    /// Sampler for sampling the shadow map and the scene depth in the resolve.
    pub sampler: Option<ash::vk::Sampler>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invert_round_trips_identity() {
        let id = [1.0,0.0,0.0,0.0, 0.0,1.0,0.0,0.0, 0.0,0.0,1.0,0.0, 0.0,0.0,0.0,1.0];
        let inv = invert(&id);
        for i in 0..16 { assert!((inv[i] - id[i]).abs() < 1e-5); }
    }

    #[test]
    fn invert_then_multiply_is_identity() {
        // A non-trivial view-proj-like matrix (the light VP).
        let m = light_view_proj([100.0, 200.0, 50.0], DEFAULT_LIGHT_DIR, 1024.0, 1500.0);
        let inv = invert(&m);
        let prod = mul(&m, &inv);
        let id = [1.0,0.0,0.0,0.0, 0.0,1.0,0.0,0.0, 0.0,0.0,1.0,0.0, 0.0,0.0,0.0,1.0];
        for i in 0..16 { assert!((prod[i] - id[i]).abs() < 1e-3, "idx {} = {}", i, prod[i]); }
    }
}
