use glam::{DMat4, DVec3, Mat4, Vec3};
use selekt::ViewState;
use terra::{Cartographic, Ellipsoid, east_north_up_to_ecef};
use zukei::SpatialBounds;

/// Globe orbit camera in WGS-84 ECEF space with ENU-relative rendering.
///
/// The camera orbits around a target point on the ellipsoid surface.
/// - Left-drag: orbit (rotate around target)
/// - Right-drag: pan (move target along surface)
/// - Scroll: zoom (change distance to target)
/// - WASD/QE: fly relative to ENU frame
pub struct FlyCamera {
    /// Camera position in ECEF metres.
    pub position_ecef: DVec3,
    /// Yaw angle in radians (rotation around ENU-Up axis, 0 = East).
    pub yaw: f64,
    /// Pitch angle in radians (0 = horizontal, negative = looking down).
    pub pitch: f64,
    /// Pending scroll zoom delta (consumed each frame).
    pub zoom_delta: f64,
}

impl FlyCamera {
    /// Default: 20,000 km above the equator, looking down.
    pub fn default(ellipsoid: &Ellipsoid) -> Self {
        let position_ecef =
            ellipsoid.cartographic_to_ecef(Cartographic::from_degrees(0.0, 0.0, 2e7));
        Self {
            position_ecef,
            yaw: 0.0,
            pitch: -0.8, // looking down toward surface
            zoom_delta: 0.0,
        }
    }

    /// Compute the ENU-to-ECEF matrix at the current camera position.
    pub fn enu_to_ecef(&self, ellipsoid: &Ellipsoid) -> DMat4 {
        east_north_up_to_ecef(self.position_ecef, ellipsoid)
    }

    /// Forward direction in ENU space derived from yaw/pitch.
    fn forward_enu(&self) -> DVec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        DVec3::new(cos_pitch * cos_yaw, cos_pitch * sin_yaw, sin_pitch)
    }

    /// Right direction in ENU space.
    fn right_enu(&self) -> DVec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        DVec3::new(sin_yaw, -cos_yaw, 0.0)
    }

    /// Move the camera by `delta` metres in ENU space.
    pub fn move_in_enu(&mut self, delta_enu: DVec3, ellipsoid: &Ellipsoid) {
        let enu_to_ecef = self.enu_to_ecef(ellipsoid);
        let delta_ecef = enu_to_ecef.transform_vector3(delta_enu);
        self.position_ecef += delta_ecef;
    }

    /// Fly forward/backward/right/left in ENU space (WASD/QE keys).
    pub fn fly(&mut self, fwd: f64, right: f64, up: f64, dt: f64, ellipsoid: &Ellipsoid) {
        let altitude = self.altitude(ellipsoid);
        let speed = altitude * 0.5 * dt;
        let forward = self.forward_enu();
        let right_dir = self.right_enu();
        let up_dir = DVec3::new(0.0, 0.0, 1.0);

        let delta = forward * (fwd * speed) + right_dir * (right * speed) + up_dir * (up * speed);
        self.move_in_enu(delta, ellipsoid);
    }

    /// Apply scroll wheel zoom: move along the look direction toward/away from surface.
    pub fn apply_zoom(&mut self, ellipsoid: &Ellipsoid) {
        if self.zoom_delta.abs() < 1e-6 {
            return;
        }
        let altitude = self.altitude(ellipsoid);
        // Scroll zoom: move along forward direction, proportional to altitude.
        let zoom_speed = altitude * 0.1 * self.zoom_delta;
        let forward = self.forward_enu();
        // When looking down, forward has a negative Z component → zoom in moves
        // toward Earth. To keep it intuitive, move along the Up direction instead
        // (negative = toward Earth, positive = away).
        let up_dir = DVec3::new(0.0, 0.0, 1.0);
        // Mix forward and up so zoom feels natural: mostly altitude change,
        // with a small forward component if the camera is tilted.
        let zoom_dir = (forward * 0.3 + up_dir * 0.7).normalize();
        self.move_in_enu(zoom_dir * zoom_speed, ellipsoid);
        // Prevent going below the surface
        let min_alt = 10.0;
        if let Some(cart) = ellipsoid.ecef_to_cartographic(self.position_ecef) {
            if cart.height < min_alt {
                self.position_ecef = ellipsoid.cartographic_to_ecef(Cartographic::new(
                    cart.longitude,
                    cart.latitude,
                    min_alt,
                ));
            }
        }
        self.zoom_delta = 0.0;
    }

    /// Orbit: left-drag rotates yaw/pitch (free-look around current position).
    pub fn rotate(&mut self, dx: f64, dy: f64) {
        const SENSITIVITY: f64 = 0.003;
        self.yaw -= dx * SENSITIVITY;
        self.pitch = (self.pitch - dy * SENSITIVITY).clamp(
            -std::f64::consts::FRAC_PI_2 + 0.01,
            std::f64::consts::FRAC_PI_2 - 0.01,
        );
    }

    /// Pan: right-drag moves the camera along the surface (East/North in ENU).
    pub fn pan(&mut self, dx: f64, dy: f64, ellipsoid: &Ellipsoid) {
        let altitude = self.altitude(ellipsoid);
        // Scale by altitude so pan rate feels consistent at any zoom level.
        // Clamp individual deltas to prevent huge spikes on first click.
        let dx = dx.clamp(-50.0, 50.0);
        let dy = dy.clamp(-50.0, 50.0);
        let scale = altitude * 0.0003;
        // dx → East movement, dy → North movement
        let delta_enu = DVec3::new(-dx * scale, dy * scale, 0.0);
        self.move_in_enu(delta_enu, ellipsoid);
    }

    fn altitude(&self, ellipsoid: &Ellipsoid) -> f64 {
        ellipsoid
            .ecef_to_cartographic(self.position_ecef)
            .map(|c| c.height)
            .unwrap_or(1e6)
            .clamp(10.0, 1e8) // cap at 100,000 km to prevent exponential blowup
    }

    /// Camera position/direction/up in ECEF for selekt `ViewState`.
    fn ecef_view_vectors(&self, ellipsoid: &Ellipsoid) -> (DVec3, DVec3, DVec3) {
        let enu_to_ecef = self.enu_to_ecef(ellipsoid);
        let forward_enu = self.forward_enu();
        let up_enu = DVec3::new(0.0, 0.0, 1.0);

        let direction_ecef = enu_to_ecef.transform_vector3(forward_enu).normalize();
        let up_ecef = enu_to_ecef.transform_vector3(up_enu).normalize();
        (self.position_ecef, direction_ecef, up_ecef)
    }

    /// Build a `ViewState` for the selekt tile selection engine.
    pub fn view_state(
        &self,
        viewport: [u32; 2],
        fov_y_rad: f64,
        ellipsoid: &Ellipsoid,
    ) -> ViewState {
        let aspect = viewport[0] as f64 / viewport[1] as f64;
        let fov_x = 2.0 * ((fov_y_rad / 2.0).tan() * aspect).atan();
        let (position, direction, up) = self.ecef_view_vectors(ellipsoid);
        ViewState::perspective(position, direction, up, viewport, fov_x, fov_y_rad)
    }

    /// Build `(proj_view_f32, ecef_to_enu_f64)` for the renderer.
    pub fn proj_view(
        &self,
        viewport: [u32; 2],
        fov_y_rad: f64,
        near: f64,
        far: f64,
        ellipsoid: &Ellipsoid,
    ) -> (Mat4, DMat4) {
        let enu_to_ecef = self.enu_to_ecef(ellipsoid);
        let ecef_to_enu = enu_to_ecef.inverse();

        let forward = self.forward_enu().as_vec3();
        let up = Vec3::new(0.0, 0.0, 1.0);
        let view = Mat4::look_to_rh(Vec3::ZERO, forward, up);

        let aspect = viewport[0] as f32 / viewport[1] as f32;
        let proj = Mat4::perspective_rh(fov_y_rad as f32, aspect, near as f32, far as f32);

        (proj * view, ecef_to_enu)
    }

    /// Position the camera above the given bounding volume.
    pub fn position_above_bounds(&mut self, bounds: &SpatialBounds, ellipsoid: &Ellipsoid) {
        let (center_ecef, radius) = match bounds {
            SpatialBounds::Sphere { center, radius } => (*center, *radius),
            SpatialBounds::OrientedBox { center, half_axes } => {
                let r = half_axes
                    .col(0)
                    .length()
                    .max(half_axes.col(1).length())
                    .max(half_axes.col(2).length())
                    * 3.0_f64.sqrt();
                (*center, r)
            }
            SpatialBounds::AxisAlignedBox { min, max } => {
                let center = (*min + *max) * 0.5;
                (center, ((*max - *min) * 0.5).length())
            }
            SpatialBounds::Rectangle { min, max } => {
                let cx = (min.x + max.x) * 0.5;
                let cy = (min.y + max.y) * 0.5;
                let r = ((max.x - min.x).hypot(max.y - min.y)) * 0.5;
                (DVec3::new(cx, cy, 0.0), r)
            }
            SpatialBounds::Polygon { vertices } => {
                if vertices.is_empty() {
                    return;
                }
                let sum = vertices.iter().fold(glam::DVec2::ZERO, |acc, v| acc + *v);
                let cx = sum.x / vertices.len() as f64;
                let cy = sum.y / vertices.len() as f64;
                (DVec3::new(cx, cy, 0.0), 1e6)
            }
        };

        // Place camera above center, offset in ENU Up by radius*2
        let enu_to_ecef = east_north_up_to_ecef(center_ecef, ellipsoid);
        let camera_enu = DVec3::new(0.0, 0.0, radius * 2.0);
        let pos_ecef = enu_to_ecef.transform_point3(camera_enu);

        // Look toward center (straight down in ENU with a slight tilt)
        self.pitch = -1.0; // ~57° below horizontal — good overview angle
        self.yaw = 0.0;
        self.position_ecef = pos_ecef;
    }
}
