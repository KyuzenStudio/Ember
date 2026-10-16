use std::collections::HashMap;
use std::sync::Mutex;

use ember_scene::{BodyKind, Entity, EntityId};
use rapier2d::prelude::{Vector, *};

pub const STEP: f32 = 1.0 / 60.0;
const MAX_STEPS: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Contact {
    pub a: EntityId,
    pub b: EntityId,
    pub begun: bool,
}

#[derive(Default)]
struct Collector {
    events: Mutex<Vec<(ColliderHandle, ColliderHandle, bool)>>,
}

impl EventHandler for Collector {
    fn handle_collision_event(
        &self,
        _bodies: &RigidBodySet,
        _colliders: &ColliderSet,
        event: CollisionEvent,
        _pair: Option<&ContactPair>,
    ) {
        let (h1, h2, begun) = match event {
            CollisionEvent::Started(h1, h2, _) => (h1, h2, true),
            CollisionEvent::Stopped(h1, h2, _) => (h1, h2, false),
        };
        self.events.lock().unwrap().push((h1, h2, begun));
    }

    fn handle_contact_force_event(
        &self,
        _dt: f32,
        _bodies: &RigidBodySet,
        _colliders: &ColliderSet,
        _pair: &ContactPair,
        _total_force: f32,
    ) {
    }

    fn handle_soft_body_tear_event(&self, _bodies: &SoftBodySet, _event: &SoftBodyTearEvent) {}
}

pub struct PhysicsWorld {
    bodies: RigidBodySet,
    colliders: ColliderSet,
    gravity: Vector,
    params: IntegrationParameters,
    islands: IslandManager,
    broad: BroadPhaseBvh,
    narrow: NarrowPhase,
    joints: ImpulseJointSet,
    multibody: MultibodyJointSet,
    soft: SoftBodySet,
    ccd: CCDSolver,
    events: Collector,
    handles: HashMap<EntityId, RigidBodyHandle>,
    by_collider: HashMap<ColliderHandle, EntityId>,
    acc: f32,
}

impl PhysicsWorld {
    pub fn build(entities: &[Entity]) -> Result<Self, String> {
        let mut world = Self {
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            gravity: Vector::new(0.0, -9.81),
            params: IntegrationParameters::default(),
            islands: IslandManager::new(),
            broad: BroadPhaseBvh::new(),
            narrow: NarrowPhase::new(),
            joints: ImpulseJointSet::new(),
            multibody: MultibodyJointSet::new(),
            soft: SoftBodySet::new(),
            ccd: CCDSolver::new(),
            events: Collector::default(),
            handles: HashMap::new(),
            by_collider: HashMap::new(),
            acc: 0.0,
        };
        for entity in entities {
            let Some(body) = &entity.rigidbody else {
                continue;
            };
            let mut builder = match body.kind {
                BodyKind::Static => RigidBodyBuilder::fixed(),
                BodyKind::Dynamic => RigidBodyBuilder::dynamic(),
            };
            builder = builder
                .translation(Vector::new(
                    entity.transform.position.x,
                    entity.transform.position.y,
                ))
                .linvel(Vector::new(body.velocity.x, body.velocity.y))
                .gravity_scale(body.gravity_scale)
                .lock_rotations();
            let handle = world.bodies.insert(builder.build());
            if let Some(collider) = &entity.collider {
                if collider.hx <= 0.0 || collider.hy <= 0.0 {
                    return Err(format!(
                        "entity '{}': collider size must be positive",
                        entity.name
                    ));
                }
                let shape = ColliderBuilder::cuboid(collider.hx, collider.hy)
                    .sensor(collider.sensor)
                    .active_events(ActiveEvents::COLLISION_EVENTS)
                    .build();
                let ch = world
                    .colliders
                    .insert_with_parent(shape, handle, &mut world.bodies);
                world.by_collider.insert(ch, entity.id);
            }
            world.handles.insert(entity.id, handle);
        }
        Ok(world)
    }

    pub fn has_bodies(&self) -> bool {
        !self.handles.is_empty()
    }

    fn sync_from(&mut self, entities: &[Entity]) {
        for entity in entities {
            if let Some(handle) = self.handles.get(&entity.id)
                && let Some(body) = self.bodies.get_mut(*handle)
            {
                body.set_translation(
                    Vector::new(entity.transform.position.x, entity.transform.position.y),
                    false,
                );
            }
        }
    }

    fn step_once(&mut self, dt: f32) {
        self.params.dt = dt;
        PhysicsPipeline::new().step(
            self.gravity,
            &self.params,
            &mut self.islands,
            &mut self.broad,
            &mut self.narrow,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.joints,
            &mut self.multibody,
            &mut self.soft,
            &mut self.ccd,
            &(),
            &self.events,
        );
    }

    pub fn take_contacts(&mut self) -> Vec<Contact> {
        self.events
            .events
            .lock()
            .unwrap()
            .drain(..)
            .filter_map(|(h1, h2, begun)| {
                let a = self.by_collider.get(&h1).copied()?;
                let b = self.by_collider.get(&h2).copied()?;
                (a != b).then_some(Contact { a, b, begun })
            })
            .collect()
    }

    fn write_back(&mut self, entities: &mut [Entity]) {
        for entity in entities.iter_mut() {
            if let Some(handle) = self.handles.get(&entity.id)
                && let Some(body) = self.bodies.get(*handle)
            {
                let pos = body.translation();
                entity.transform.position.x = pos.x;
                entity.transform.position.y = pos.y;
                if let Some(rigid) = &mut entity.rigidbody {
                    let vel = body.linvel();
                    rigid.velocity.x = vel.x;
                    rigid.velocity.y = vel.y;
                }
            }
        }
    }

    pub fn tick(&mut self, entities: &mut [Entity], dt: f32) {
        self.sync_from(entities);
        self.acc = (self.acc + dt).min(STEP * MAX_STEPS as f32);
        while self.acc >= STEP {
            self.step_once(STEP);
            self.acc -= STEP;
        }
        self.write_back(entities);
    }
}

impl std::fmt::Debug for PhysicsWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhysicsWorld")
            .field("bodies", &self.handles.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ember_core::Vec2;
    use ember_scene::{Collider2D, RigidBody2D, Scene};

    fn fall_scene() -> Scene {
        let mut scene = Scene::new("fall");
        let ground = scene.spawn("Ground");
        let ground_entity = scene.entities.iter_mut().find(|e| e.id == ground).unwrap();
        ground_entity.transform.position = Vec2::new(0.0, -1.0);
        ground_entity.rigidbody = Some(RigidBody2D {
            kind: BodyKind::Static,
            velocity: Vec2::zero(),
            gravity_scale: 1.0,
        });
        ground_entity.collider = Some(Collider2D {
            hx: 10.0,
            hy: 1.0,
            sensor: false,
        });
        let cube = scene.spawn("Cube");
        let cube_entity = scene.entities.iter_mut().find(|e| e.id == cube).unwrap();
        cube_entity.transform.position = Vec2::new(0.0, 5.0);
        cube_entity.rigidbody = Some(RigidBody2D {
            kind: BodyKind::Dynamic,
            velocity: Vec2::zero(),
            gravity_scale: 1.0,
        });
        cube_entity.collider = Some(Collider2D {
            hx: 0.5,
            hy: 0.5,
            sensor: false,
        });
        scene
    }

    fn y_of(scene: &Scene, name: &str) -> f32 {
        scene
            .entities
            .iter()
            .find(|e| e.name == name)
            .unwrap()
            .transform
            .position
            .y
    }

    #[test]
    fn box_falls_and_rests() {
        let mut scene = fall_scene();
        let mut world = PhysicsWorld::build(&scene.entities).unwrap();
        assert!(world.has_bodies());
        for _ in 0..180 {
            world.tick(&mut scene.entities, 1.0 / 60.0);
        }
        let y = y_of(&scene, "Cube");
        assert!((y - 0.5).abs() < 0.05, "rest y = {y}");
        assert_eq!(y_of(&scene, "Ground"), -1.0);
    }

    #[test]
    fn velocity_written_back() {
        let mut scene = fall_scene();
        let mut world = PhysicsWorld::build(&scene.entities).unwrap();
        world.tick(&mut scene.entities, 1.0 / 60.0);
        let vel = scene
            .entities
            .iter()
            .find(|e| e.name == "Cube")
            .unwrap()
            .rigidbody
            .as_ref()
            .unwrap()
            .velocity;
        assert!(vel.y < -0.05, "vel = {vel:?}");
    }

    #[test]
    fn bad_collider_rejected() {
        let mut scene = Scene::new("bad");
        let id = scene.spawn("Bad");
        let entity = scene.entities.iter_mut().find(|e| e.id == id).unwrap();
        entity.rigidbody = Some(RigidBody2D {
            kind: BodyKind::Dynamic,
            velocity: Vec2::zero(),
            gravity_scale: 1.0,
        });
        entity.collider = Some(Collider2D {
            hx: 0.0,
            hy: 1.0,
            sensor: false,
        });
        let err = PhysicsWorld::build(&scene.entities).unwrap_err();
        assert!(err.contains("must be positive"));
    }

    #[test]
    fn substep_dt_accumulates() {
        let mut scene = Scene::new("fast");
        let id = scene.spawn("Cube");
        let entity = scene.entities.iter_mut().find(|e| e.id == id).unwrap();
        entity.transform.position = Vec2::new(0.0, 5.0);
        entity.rigidbody = Some(RigidBody2D {
            kind: BodyKind::Dynamic,
            velocity: ember_core::Vec2::zero(),
            gravity_scale: 1.0,
        });
        entity.collider = Some(Collider2D {
            hx: 0.5,
            hy: 0.5,
            sensor: false,
        });
        let mut world = PhysicsWorld::build(&scene.entities).unwrap();
        for _ in 0..240 {
            world.tick(&mut scene.entities, 1.0 / 600.0);
        }
        let y = y_of(&scene, "Cube");
        assert!(y < 4.9, "sub-step time was dropped: y = {y}");
    }

    #[test]
    fn no_bodies_without_rigidbody() {
        let mut scene = Scene::new("empty");
        scene.spawn("Plain");
        let world = PhysicsWorld::build(&scene.entities).unwrap();
        assert!(!world.has_bodies());
    }
}
