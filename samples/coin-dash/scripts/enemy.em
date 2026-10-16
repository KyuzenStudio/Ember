var dir: f32 = 1.0

fn on_update(dt: f32) {
    transform.move(dir * 3.0 * dt, 0.0)
    if transform.x() > 9.0 {
        dir = -1.0
    }
    if transform.x() < 7.0 {
        dir = 1.0
    }
}
